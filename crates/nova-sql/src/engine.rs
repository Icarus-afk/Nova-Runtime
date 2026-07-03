use std::sync::Arc;
use std::time::Instant;

use crate::ast::*;
use crate::binder::Binder;
use crate::config::SQLConfig;
use crate::error::{Result, SQLError};
use crate::execute::executor::build_executor;
use crate::execute::table_store::{Row, TableStore};
use crate::execute::evaluate_expr;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::plan::planner::LogicalPlanner;
use crate::result::{Column, ExecutionStats, RecordBatch};
use crate::schema::{ColumnInfo, Schema};

pub struct SQLEngine {
    #[allow(dead_code)]
    config: SQLConfig,
    tables: Arc<TableStore>,
}

impl SQLEngine {
    pub fn new(config: SQLConfig) -> Self {
        SQLEngine {
            config,
            tables: Arc::new(TableStore::new()),
        }
    }

    pub fn execute(&self, sql: &str) -> Result<SQLResult> {
        let start = Instant::now();
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let statements = parser.parse_program()?;

        let mut final_result = None;
        for stmt in statements {
            final_result = Some(self.execute_statement(stmt, &start)?);
        }

        final_result.ok_or_else(|| SQLError::Syntax("empty statement".to_string()))
    }

    pub fn execute_query(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        match self.execute(sql)? {
            SQLResult::Query { batches, .. } => Ok(batches),
            SQLResult::Exec { .. } => {
                Err(SQLError::Syntax("query did not return results".to_string()))
            }
        }
    }

    fn execute_statement(&self, stmt: Statement, start: &Instant) -> Result<SQLResult> {
        match stmt {
            Statement::Select(sel) => self.execute_select(sel, start),
            Statement::Insert(ins) => self.execute_insert(ins, start),
            Statement::Update(upd) => self.execute_update(upd, start),
            Statement::Delete(del) => self.execute_delete(del, start),
            Statement::CreateTable(ct) => self.execute_create_table(ct, start),
            Statement::DropTable(dt) => self.execute_drop_table(dt, start),
        }
    }

    fn execute_create_table(
        &self,
        stmt: CreateTableStatement,
        start: &Instant,
    ) -> Result<SQLResult> {
        let columns: Vec<ColumnInfo> = stmt
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ColumnInfo {
                name: c.name.clone(),
                sql_type: c.sql_type.clone(),
                nullable: c.nullable,
                ordinal: i,
            })
            .collect();
        let schema = Schema::new(columns);
        self.tables.create_table(&stmt.table.name, schema)?;
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected: 0,
            stats: ExecutionStats::new(0, 0, elapsed),
        })
    }

    fn execute_drop_table(
        &self,
        stmt: DropTableStatement,
        start: &Instant,
    ) -> Result<SQLResult> {
        self.tables.drop_table(&stmt.table.name)?;
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected: 0,
            stats: ExecutionStats::new(0, 0, elapsed),
        })
    }

    fn execute_insert(
        &self,
        stmt: InsertStatement,
        start: &Instant,
    ) -> Result<SQLResult> {
        let schema = self.tables.get_schema(&stmt.table.name)?;

        // Resolve column indices
        let col_indices: Vec<usize> = if stmt.columns.is_empty() {
            (0..schema.len()).collect()
        } else {
            stmt.columns
                .iter()
                .map(|c| {
                    schema
                        .find_index(c)
                        .ok_or_else(|| SQLError::ColumnNotFound(c.clone()))
                })
                .collect::<Result<Vec<_>>>()?
        };

        let num_cols = col_indices.len();
        let mut rows_inserted = 0u64;

        for value_row in &stmt.values {
            if value_row.len() != num_cols {
                return Err(SQLError::Syntax(format!(
                    "expected {} values, got {}",
                    num_cols,
                    value_row.len()
                )));
            }
            let mut row_values: Vec<Option<LiteralValue>> = vec![None; schema.len()];
            let _binder = Binder::new();

            for (j, expr) in value_row.iter().enumerate() {
                let col_idx = col_indices[j];
                let col_info = &schema.columns[col_idx];
                // Bind/resolve column references in expression (might reference other columns)
                match expr {
                    Expr::Column(name) => {
                        if schema.find_column(name).is_none() {
                            return Err(SQLError::ColumnNotFound(name.clone()));
                        }
                    }
                    _ => {}
                }
                let empty_row = vec![None; schema.len()];
                let val = evaluate_expr(expr, &empty_row, &schema)?;
                let val = coerce_insert_value(val, &col_info.sql_type)?;
                row_values[col_idx] = Some(val);
            }
            self.tables.insert_row(&stmt.table.name, Row::new(row_values))?;
            rows_inserted += 1;
        }

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected: rows_inserted,
            stats: ExecutionStats::new(0, rows_inserted, elapsed),
        })
    }

    fn execute_select(
        &self,
        mut stmt: SelectStatement,
        start: &Instant,
    ) -> Result<SQLResult> {
        let schema = self.tables.get_schema(&stmt.from.name)?;

        // Check table existence
        if !self.tables.table_exists(&stmt.from.name) {
            return Err(SQLError::TableNotFound(stmt.from.name.clone()));
        }

        // Expand wildcards
        stmt.select_list = expand_wildcards(&stmt.select_list, &schema);

        // Bind and type check
        let binder = Binder::new();
        let _bound = binder.bind(&stmt, &schema)?;

        // Create logical plan
        let planner = LogicalPlanner::new();
        let plan = planner.plan_select(stmt);

        // Build and execute
        let mut executor = build_executor(&plan, self.tables.clone())?;
        executor.open()?;

        let mut rows: Vec<Row> = Vec::new();
        while let Some(row) = executor.next()? {
            rows.push(row);
        }
        executor.close()?;

        // Convert rows to RecordBatch
        let batch = rows_to_record_batch(&rows);
        let num_rows = batch.num_rows;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Query {
            batches: vec![batch],
            stats: ExecutionStats::new(rows.len() as u64, num_rows as u64, elapsed),
        })
    }

    fn execute_update(
        &self,
        stmt: UpdateStatement,
        start: &Instant,
    ) -> Result<SQLResult> {
        let schema = self.tables.get_schema(&stmt.table.name)?;
        let mut rows = self.tables.scan_rows(&stmt.table.name)?;
        let mut rows_affected = 0u64;

        for row in &mut rows {
            // Evaluate WHERE clause
            if let Some(ref predicate) = stmt.where_clause {
                let result = evaluate_expr(predicate, &row.values, &schema)?;
                if result != LiteralValue::Boolean(true) {
                    continue;
                }
            }

            // Apply assignments
            for assignment in &stmt.assignments {
                let idx = schema
                    .find_index(&assignment.column)
                    .ok_or_else(|| SQLError::ColumnNotFound(assignment.column.clone()))?;
                let val = evaluate_expr(&assignment.value, &row.values, &schema)?;
                let val = coerce_insert_value(val, &schema.columns[idx].sql_type)?;
                row.values[idx] = Some(val);
            }
            rows_affected += 1;
        }

        // Write back
        self.tables.drop_table(&stmt.table.name)?;
        let columns: Vec<ColumnInfo> = schema.columns.clone();
        let new_schema = Schema::new(columns);
        self.tables.create_table(&stmt.table.name, new_schema)?;
        self.tables.insert_rows(&stmt.table.name, rows)?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected,
            stats: ExecutionStats::new(0, rows_affected, elapsed),
        })
    }

    fn execute_delete(
        &self,
        stmt: DeleteStatement,
        start: &Instant,
    ) -> Result<SQLResult> {
        let schema = self.tables.get_schema(&stmt.table.name)?;
        let rows = self.tables.scan_rows(&stmt.table.name)?;
        let mut rows_affected = 0u64;

        let kept_rows: Vec<Row> = if let Some(ref predicate) = stmt.where_clause {
            rows.into_iter()
                .filter(|row| {
                    let result = evaluate_expr(predicate, &row.values, &schema);
                    match result {
                        Ok(LiteralValue::Boolean(true)) => {
                            rows_affected += 1;
                            false
                        }
                        _ => true,
                    }
                })
                .collect()
        } else {
            rows_affected = rows.len() as u64;
            Vec::new()
        };

        // Write back
        self.tables.drop_table(&stmt.table.name)?;
        let columns: Vec<ColumnInfo> = schema.columns.clone();
        let new_schema = Schema::new(columns);
        self.tables.create_table(&stmt.table.name, new_schema)?;
        self.tables.insert_rows(&stmt.table.name, kept_rows)?;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected,
            stats: ExecutionStats::new(0, rows_affected, elapsed),
        })
    }
}

fn expand_wildcards(items: &[SelectItem], schema: &Schema) -> Vec<SelectItem> {
    let mut result = Vec::new();
    for item in items {
        match item {
            SelectItem::Wildcard => {
                for col in &schema.columns {
                    result.push(SelectItem::Expr {
                        expr: Expr::Column(col.name.clone()),
                        alias: None,
                    });
                }
            }
            other => result.push(other.clone()),
        }
    }
    result
}

fn rows_to_record_batch(rows: &[Row]) -> RecordBatch {
    if rows.is_empty() {
        return RecordBatch::new(vec![], 0);
    }
    let num_cols = rows[0].values.len();
    let num_rows = rows.len();

    let mut col_types: Vec<Option<SQLType>> = vec![None; num_cols];
    for row in rows {
        for (i, val) in row.values.iter().enumerate() {
            if col_types[i].is_none() {
                if let Some(v) = val {
                    col_types[i] = Some(match v {
                        LiteralValue::Null => continue,
                        LiteralValue::Boolean(_) => SQLType::Boolean,
                        LiteralValue::Integer(_) => SQLType::Integer,
                        LiteralValue::Float(_) => SQLType::Float,
                        LiteralValue::String(_) => SQLType::Text,
                    });
                }
            }
        }
    }

    let mut columns: Vec<Column> = col_types
        .iter()
        .map(|t| match t {
            Some(SQLType::Integer) => Column::Integer(Vec::with_capacity(num_rows)),
            Some(SQLType::Float) => Column::Float(Vec::with_capacity(num_rows)),
            Some(SQLType::Boolean) => Column::Boolean(Vec::with_capacity(num_rows)),
            Some(SQLType::Text) => Column::String(Vec::with_capacity(num_rows)),
            _ => Column::Null(num_rows),
        })
        .collect();

    for row in rows {
        for (i, val) in row.values.iter().enumerate() {
            if i >= columns.len() {
                continue;
            }
            let opt_val = val.clone().map(|v| if matches!(v, LiteralValue::Null) { None } else { Some(v) }).flatten();
            push_value_to_column(&mut columns[i], opt_val);
        }
    }

    RecordBatch::new(columns, num_rows)
}

fn push_value_to_column(col: &mut Column, val: Option<LiteralValue>) {
    match col {
        Column::Integer(v) => v.push(val.map(|x| match x {
            LiteralValue::Integer(i) => i,
            LiteralValue::Float(f) => f as i64,
            _ => 0,
        })),
        Column::Float(v) => v.push(val.map(|x| match x {
            LiteralValue::Float(f) => f,
            LiteralValue::Integer(i) => i as f64,
            _ => 0.0,
        })),
        Column::Boolean(v) => v.push(val.map(|x| match x {
            LiteralValue::Boolean(b) => b,
            _ => false,
        })),
        Column::String(v) => v.push(val.map(|x| match x {
            LiteralValue::String(s) => s,
            _ => format!("{:?}", x),
        })),
        Column::Null(n) => *n += 1,
    }
}

fn coerce_insert_value(val: LiteralValue, target: &SQLType) -> Result<LiteralValue> {
    use crate::type_checker::TypeChecker;
    TypeChecker::coerce_value(&val, target)
}

#[derive(Debug)]
pub enum SQLResult {
    Query {
        batches: Vec<RecordBatch>,
        stats: ExecutionStats,
    },
    Exec {
        rows_affected: u64,
        stats: ExecutionStats,
    },
}
