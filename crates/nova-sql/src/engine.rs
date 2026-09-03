use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use parking_lot::RwLock;

use nova_core::{Key, StorageEngine, Value};
use serde::{Deserialize, Serialize};

use crate::ast::*;
use crate::binder::Binder;
use crate::config::SQLConfig;
use crate::error::{Result, SQLError};
use crate::execute::evaluate_expr;
use crate::execute::executor::build_executor;
use crate::execute::table_store::{Row, TableStore};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::plan::planner::LogicalPlanner;
use crate::result::{Column, ExecutionStats, RecordBatch};
use crate::schema::{ColumnInfo, Schema};

#[derive(Serialize, Deserialize)]
struct PersistedTable {
    schema: Schema,
    rows: Vec<Row>,
}

#[derive(Serialize, Deserialize)]
struct PersistedTableMeta {
    schema: Schema,
    chunk_count: usize,
}

// WAL payload limit is 65535; keep value well under that.
const MAX_PERSIST_VALUE_SIZE: usize = 60000;

fn table_chunk_key(name: &str, idx: usize) -> Key {
    Key::from(format!("sql:table:{name}:chunk:{idx}").as_bytes().to_vec())
}

pub trait MutationObserver: Send + Sync {
    fn on_table_created(&self, _table: &str) {}
    fn on_table_dropped(&self, _table: &str) {}
    fn on_rows_inserted(&self, _table: &str, _count: u64) {}
    fn on_rows_updated(&self, _table: &str, _count: u64) {}
    fn on_rows_deleted(&self, _table: &str, _count: u64) {}
}

pub struct SQLEngineMutationObserver {
    bus: Arc<nova_event::EventBus>,
}

impl SQLEngineMutationObserver {
    pub fn new(bus: Arc<nova_event::EventBus>) -> Self {
        SQLEngineMutationObserver { bus }
    }
}

impl MutationObserver for SQLEngineMutationObserver {
    fn on_table_created(&self, table: &str) {
        use nova_event::{EventBuilder, Subsystem};
        let payload = serde_json::json!({"table": table});
        let event = EventBuilder::new("db.table.created")
            .unwrap()
            .source(Subsystem::Storage, "sql", "local", "default")
            .build(serde_json::to_vec(&payload).unwrap_or_default());
        let _ = self.bus.publish(event);
    }

    fn on_table_dropped(&self, table: &str) {
        use nova_event::{EventBuilder, Subsystem};
        let payload = serde_json::json!({"table": table});
        let event = EventBuilder::new("db.table.dropped")
            .unwrap()
            .source(Subsystem::Storage, "sql", "local", "default")
            .build(serde_json::to_vec(&payload).unwrap_or_default());
        let _ = self.bus.publish(event);
    }

    fn on_rows_inserted(&self, table: &str, count: u64) {
        use nova_event::{EventBuilder, Subsystem};
        let payload = serde_json::json!({"table": table, "count": count});
        let event = EventBuilder::new("db.table.insert")
            .unwrap()
            .source(Subsystem::Storage, "sql", "local", "default")
            .build(serde_json::to_vec(&payload).unwrap_or_default());
        let _ = self.bus.publish(event);
    }

    fn on_rows_updated(&self, table: &str, count: u64) {
        use nova_event::{EventBuilder, Subsystem};
        let payload = serde_json::json!({"table": table, "count": count});
        let event = EventBuilder::new("db.table.update")
            .unwrap()
            .source(Subsystem::Storage, "sql", "local", "default")
            .build(serde_json::to_vec(&payload).unwrap_or_default());
        let _ = self.bus.publish(event);
    }

    fn on_rows_deleted(&self, table: &str, count: u64) {
        use nova_event::{EventBuilder, Subsystem};
        let payload = serde_json::json!({"table": table, "count": count});
        let event = EventBuilder::new("db.table.delete")
            .unwrap()
            .source(Subsystem::Storage, "sql", "local", "default")
            .build(serde_json::to_vec(&payload).unwrap_or_default());
        let _ = self.bus.publish(event);
    }
}

pub struct SQLEngine {
    #[allow(dead_code)]
    config: SQLConfig,
    tables: Arc<TableStore>,
    shutdown: Arc<AtomicBool>,
    storage: Option<Arc<dyn StorageEngine>>,
    observer: Mutex<Option<Arc<dyn MutationObserver>>>,
    engine_lock: RwLock<()>,
}

impl SQLEngine {
    pub fn new(config: SQLConfig) -> Self {
        SQLEngine {
            config,
            tables: Arc::new(TableStore::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
            storage: None,
            observer: Mutex::new(None),
            engine_lock: RwLock::new(()),
        }
    }

    pub fn new_with_storage(config: SQLConfig, storage: Arc<dyn StorageEngine>) -> Self {
        let engine = SQLEngine {
            config,
            tables: Arc::new(TableStore::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
            storage: Some(storage),
            observer: Mutex::new(None),
            engine_lock: RwLock::new(()),
        };
        if let Err(e) = engine.load_tables() {
            tracing::warn!("Failed to load SQL tables from storage: {e}");
        }
        engine
    }

    pub fn set_observer(&self, observer: Arc<dyn MutationObserver>) {
        if let Ok(mut guard) = self.observer.lock() {
            *guard = Some(observer);
        }
    }

    fn notify(&self, f: impl FnOnce(&dyn MutationObserver)) {
        if let Ok(guard) = self.observer.lock()
            && let Some(ref obs) = *guard
        {
            f(obs.as_ref());
        }
    }

    fn table_key(name: &str) -> Key {
        Key::from(format!("sql:table:{name}").as_bytes().to_vec())
    }

    fn table_names_key() -> Key {
        Key::from(b"sql:table_names".to_vec())
    }

    fn persist_table(&self, name: &str) -> Result<()> {
        let Some(ref storage) = self.storage else {
            return Ok(());
        };
        let schema = self
            .tables
            .get_schema(name)
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        let rows = self
            .tables
            .scan_rows(name)
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        let persisted = PersistedTable {
            schema: schema.clone(),
            rows: rows.clone(),
        };
        let json = serde_json::to_vec(&persisted).map_err(|e| SQLError::syntax(e.to_string()))?;
        // Small table: single key, delete any old chunk artefacts.
        if json.len() <= MAX_PERSIST_VALUE_SIZE {
            let old_count = Self::existing_chunk_count(storage, name);
            if old_count == 0 {
                storage
                    .set(&Self::table_key(name), Value::new(json))
                    .map_err(|e| SQLError::syntax(e.to_string()))?;
            } else {
                let mut ops = vec![nova_core::WriteOperation::Set {
                    key: Self::table_key(name),
                    value: Value::new(json),
                }];
                for idx in 0..old_count {
                    ops.push(nova_core::WriteOperation::Delete {
                        key: table_chunk_key(name, idx),
                    });
                }
                storage
                    .batch(ops)
                    .map_err(|e| SQLError::syntax(e.to_string()))?;
            }
            return Ok(());
        }

        // Chunked persistence: split rows so each chunk fits in WAL limit.
        let mut chunks: Vec<Vec<Row>> = Vec::new();
        let mut cur: Vec<Row> = Vec::new();
        for row in rows {
            cur.push(row);
            let chunk_json =
                serde_json::to_vec(&cur).map_err(|e| SQLError::syntax(e.to_string()))?;
            if chunk_json.len() > MAX_PERSIST_VALUE_SIZE {
                if cur.len() == 1 {
                    return Err(SQLError::syntax("single row too large to persist"));
                }
                let last = cur.pop().unwrap();
                let flushed = std::mem::take(&mut cur);
                chunks.push(flushed);
                cur.push(last);
                // Validate single row still fits (should after pop)
                let check = serde_json::to_vec(&cur).map_err(|e| SQLError::syntax(e.to_string()))?;
                if check.len() > MAX_PERSIST_VALUE_SIZE {
                    return Err(SQLError::syntax("single row too large to persist"));
                }
            }
        }
        if !cur.is_empty() {
            chunks.push(cur);
        }
        if chunks.is_empty() {
            // Empty table: still write meta with 0 chunks, delete old chunks
            let meta = PersistedTableMeta {
                schema,
                chunk_count: 0,
            };
            let meta_json =
                serde_json::to_vec(&meta).map_err(|e| SQLError::syntax(e.to_string()))?;
            let old_count = Self::existing_chunk_count(storage, name);
            let mut ops = vec![nova_core::WriteOperation::Set {
                key: Self::table_key(name),
                value: Value::new(meta_json),
            }];
            for idx in 0..old_count {
                ops.push(nova_core::WriteOperation::Delete {
                    key: table_chunk_key(name, idx),
                });
            }
            storage
                .batch(ops)
                .map_err(|e| SQLError::syntax(e.to_string()))?;
            return Ok(());
        }

        let meta = PersistedTableMeta {
            schema,
            chunk_count: chunks.len(),
        };
        let meta_json =
            serde_json::to_vec(&meta).map_err(|e| SQLError::syntax(e.to_string()))?;
        if meta_json.len() > MAX_PERSIST_VALUE_SIZE {
            return Err(SQLError::syntax("schema too large to persist"));
        }
        let old_count = Self::existing_chunk_count(storage, name);
        let mut ops = Vec::with_capacity(1 + chunks.len() + old_count);
        ops.push(nova_core::WriteOperation::Set {
            key: Self::table_key(name),
            value: Value::new(meta_json),
        });
        for (idx, chunk) in chunks.iter().enumerate() {
            let chunk_json =
                serde_json::to_vec(chunk).map_err(|e| SQLError::syntax(e.to_string()))?;
            ops.push(nova_core::WriteOperation::Set {
                key: table_chunk_key(name, idx),
                value: Value::new(chunk_json),
            });
        }
        for idx in chunks.len()..old_count {
            ops.push(nova_core::WriteOperation::Delete {
                key: table_chunk_key(name, idx),
            });
        }
        storage
            .batch(ops)
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        Ok(())
    }

    fn existing_chunk_count(storage: &Arc<dyn StorageEngine>, name: &str) -> usize {
        let Ok(Some(val)) = storage.get(&Self::table_key(name)) else {
            return 0;
        };
        if let Ok(meta) = serde_json::from_slice::<PersistedTableMeta>(val.as_bytes()) {
            return meta.chunk_count;
        }
        0
    }

    fn restore_auto_increment(tables: &TableStore, name: &str, schema: &Schema) -> Result<()> {
        for (idx, col) in schema.columns.iter().enumerate() {
            if col.auto_increment {
                let rows = tables
                    .scan_rows(name)
                    .map_err(|e| SQLError::syntax(e.to_string()))?;
                let mut max: u64 = 0;
                for row in rows {
                    if let Some(Some(LiteralValue::Integer(v))) = row.values.get(idx) {
                        let uv = *v as u64;
                        if uv > max {
                            max = uv;
                        }
                    }
                }
                if max > 0 {
                    tables
                        .ensure_next_row_id(name, max)
                        .map_err(|e| SQLError::syntax(e.to_string()))?;
                }
            }
        }
        Ok(())
    }

    fn persist_table_names(&self) -> Result<()> {
        let Some(ref storage) = self.storage else {
            return Ok(());
        };
        let names = self.tables.table_names();
        let json = serde_json::to_vec(&names).map_err(|e| SQLError::syntax(e.to_string()))?;
        if json.len() > MAX_PERSIST_VALUE_SIZE {
            return Err(SQLError::syntax("too many tables to persist in one value"));
        }
        storage
            .set(&Self::table_names_key(), Value::new(json))
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        Ok(())
    }

    fn persist_create_table(&self, name: &str) -> Result<()> {
        if self.storage.is_none() {
            return Ok(());
        }
        // Try to make persist_table + persist_table_names atomic via batch.
        // persist_table already does its own batch; we now ensure table_names is also durable.
        // Order: table data first (via persist_table) then names. If we can, do both in one batch
        // for the small-table case; for chunked we fall back to sequential but table data is already
        // batched. Orphan on crash after table but before names is invisible (table not listed).
        self.persist_table(name)?;
        self.persist_table_names()
    }

    fn persist_drop_table(&self, name: &str) -> Result<()> {
        let Some(ref storage) = self.storage else {
            return Ok(());
        };
        let names = self.tables.table_names();
        let json = serde_json::to_vec(&names).map_err(|e| SQLError::syntax(e.to_string()))?;
        let old_count = Self::existing_chunk_count(storage, name);
        let mut ops = vec![nova_core::WriteOperation::Delete {
            key: Self::table_key(name),
        }];
        for idx in 0..old_count {
            ops.push(nova_core::WriteOperation::Delete {
                key: table_chunk_key(name, idx),
            });
        }
        ops.push(nova_core::WriteOperation::Set {
            key: Self::table_names_key(),
            value: Value::new(json),
        });
        storage
            .batch(ops)
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        Ok(())
    }

    fn load_tables(&self) -> Result<()> {
        let Some(ref storage) = self.storage else {
            return Ok(());
        };
        let raw = storage
            .get(&Self::table_names_key())
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        let Some(val) = raw else { return Ok(()) };
        let names: Vec<String> =
            serde_json::from_slice(val.as_bytes()).map_err(|e| SQLError::syntax(e.to_string()))?;
        for name in &names {
            let raw = storage
                .get(&Self::table_key(name))
                .map_err(|e| SQLError::syntax(e.to_string()))?;
            let Some(val) = raw else { continue };
            // Try old single-key format first.
            if let Ok(pt) = serde_json::from_slice::<PersistedTable>(val.as_bytes()) {
                let schema_clone = pt.schema.clone();
                self.tables
                    .create_table(name, pt.schema)
                    .map_err(|e| SQLError::syntax(e.to_string()))?;
                for row in pt.rows {
                    self.tables
                        .insert_row(name, row)
                        .map_err(|e| SQLError::syntax(e.to_string()))?;
                }
                Self::restore_auto_increment(&self.tables, name, &schema_clone)?;
                continue;
            }
            // Try chunked meta format.
            if let Ok(meta) = serde_json::from_slice::<PersistedTableMeta>(val.as_bytes()) {
                let schema_clone = meta.schema.clone();
                self.tables
                    .create_table(name, meta.schema)
                    .map_err(|e| SQLError::syntax(e.to_string()))?;
                for idx in 0..meta.chunk_count {
                    let raw = storage
                        .get(&table_chunk_key(name, idx))
                        .map_err(|e| SQLError::syntax(e.to_string()))?;
                    if let Some(cval) = raw {
                        let chunk: Vec<Row> = serde_json::from_slice(cval.as_bytes())
                            .map_err(|e| SQLError::syntax(e.to_string()))?;
                        for row in chunk {
                            self.tables
                                .insert_row(name, row)
                                .map_err(|e| SQLError::syntax(e.to_string()))?;
                        }
                    }
                }
                Self::restore_auto_increment(&self.tables, name, &schema_clone)?;
                continue;
            }
            return Err(SQLError::syntax(format!(
                "corrupt persisted table {name}"
            )));
        }
        Ok(())
    }

    fn persist_all(&self) -> Result<()> {
        let names = self.tables.table_names();
        for name in &names {
            self.persist_table(name)?;
        }
        self.persist_table_names()?;
        Ok(())
    }

    pub fn table_names(&self) -> Vec<String> {
        self.tables.table_names()
    }

    pub fn get_table_schema(&self, name: &str) -> Result<Schema> {
        self.tables.get_schema(name)
    }

    pub fn num_rows(&self, name: &str) -> Result<usize> {
        self.tables.num_rows(name)
    }

    pub fn shutdown(&self) {
        // Persist all tables before shutdown
        let _ = self.persist_all();
        self.shutdown.store(true, Ordering::Relaxed);
    }

    pub fn execute(&self, sql: &str) -> Result<SQLResult> {
        let start = Instant::now();
        let mut lexer = Lexer::new(sql);
        let (tokens, positions) = lexer.tokenize()?;
        let mut parser = Parser::new(tokens, positions);
        let statements = parser.parse_program()?;

        let mut final_result = None;
        for stmt in statements {
            final_result = Some(self.execute_statement(stmt, &start)?);
        }

        final_result.ok_or_else(|| SQLError::syntax("empty statement"))
    }

    pub fn execute_query(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        match self.execute(sql)? {
            SQLResult::Query { batches, .. } => Ok(batches),
            SQLResult::Exec { .. } => Err(SQLError::syntax("query did not return results")),
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
        let _guard = self.engine_lock.write();
        let columns: Vec<ColumnInfo> = stmt
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ColumnInfo {
                name: c.name.clone(),
                sql_type: c.sql_type.clone(),
                nullable: c.nullable,
                default: c.default.clone(),
                ordinal: i,
                unique: c.unique || c.is_primary_key,
                is_primary_key: c.is_primary_key,
                auto_increment: c.auto_increment,
            })
            .collect();
        let schema = Schema::new(columns);
        self.tables.create_table(&stmt.table.name, schema)?;
        self.persist_create_table(&stmt.table.name)
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        self.notify(|obs| obs.on_table_created(&stmt.table.name));
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected: 0,
            stats: ExecutionStats::new(0, 0, elapsed),
        })
    }

    fn execute_drop_table(&self, stmt: DropTableStatement, start: &Instant) -> Result<SQLResult> {
        let _guard = self.engine_lock.write();
        self.tables.drop_table(&stmt.table.name)?;
        self.persist_drop_table(&stmt.table.name)
            .map_err(|e| SQLError::syntax(e.to_string()))?;
        self.notify(|obs| obs.on_table_dropped(&stmt.table.name));
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected: 0,
            stats: ExecutionStats::new(0, 0, elapsed),
        })
    }

    fn execute_insert(&self, stmt: InsertStatement, start: &Instant) -> Result<SQLResult> {
        let _guard = self.engine_lock.write();
        let schema = self.tables.get_schema(&stmt.table.name)?;

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
                return Err(SQLError::syntax(format!(
                    "expected {} values, got {}",
                    num_cols,
                    value_row.len()
                )));
            }
            let mut row_values: Vec<Option<LiteralValue>> = vec![None; schema.len()];

            for (j, expr) in value_row.iter().enumerate() {
                let col_idx = col_indices[j];
                let col_info = &schema.columns[col_idx];
                if let Expr::Column(name) = expr
                    && schema.find_column(name).is_none()
                {
                    return Err(SQLError::ColumnNotFound(name.clone()));
                }
                let empty_row = vec![None; schema.len()];
                let val = evaluate_expr(expr, &empty_row, &schema)?;
                let val = coerce_insert_value(val, &col_info.sql_type)?;
                row_values[col_idx] = Some(val);
            }

            // Apply DEFAULT for missing columns
            for (col_idx, col_info) in schema.columns.iter().enumerate() {
                if row_values[col_idx].is_none()
                    && let Some(ref default_val) = col_info.default
                {
                    row_values[col_idx] = Some(default_val.clone());
                }
            }

            // Handle AUTO_INCREMENT: generate id if null/missing
            for (col_idx, col_info) in schema.columns.iter().enumerate() {
                if col_info.auto_increment {
                    let is_null = row_values[col_idx].is_none()
                        || row_values[col_idx]
                            .as_ref()
                            .is_some_and(|v| *v == LiteralValue::Null);
                    if is_null {
                        // Use peek of next_row_id; insert_row will bump it by 1.
                        let id = self.tables.current_row_id(&stmt.table.name)? + 1;
                        row_values[col_idx] = Some(LiteralValue::Integer(id as i64));
                    }
                }
            }

            // Enforce NOT NULL constraints
            for (col_idx, col_info) in schema.columns.iter().enumerate() {
                let is_null = row_values[col_idx].is_none()
                    || row_values[col_idx]
                        .as_ref()
                        .is_some_and(|v| *v == LiteralValue::Null);
                if !col_info.nullable && is_null {
                    return Err(SQLError::ConstraintViolation(format!(
                        "column '{}' cannot be null",
                        col_info.name
                    )));
                }
            }

            // Enforce UNIQUE constraints (including PRIMARY KEY)
            for (col_idx, col_info) in schema.columns.iter().enumerate() {
                if (col_info.unique || col_info.is_primary_key)
                    && let Some(ref val) = row_values[col_idx]
                {
                    let existing = self.tables.scan_rows(&stmt.table.name)?;
                    for row in &existing {
                        if let Some(Some(existing_val)) = row.values.get(col_idx)
                            && existing_val == val
                        {
                            return Err(SQLError::ConstraintViolation(format!(
                                "duplicate value for unique column '{}'",
                                col_info.name
                            )));
                        }
                    }
                }
            }

            let persisted_row = Row::new(row_values.clone());
            self.tables
                .insert_row(&stmt.table.name, persisted_row)?;
            // If an explicit auto_increment value exceeds the counter, ensure next id tracks it.
            for (col_idx, col_info) in schema.columns.iter().enumerate() {
                if col_info.auto_increment
                    && let Some(Some(LiteralValue::Integer(explicit))) = row_values.get(col_idx)
                {
                    let cur = self.tables.current_row_id(&stmt.table.name)?;
                    if (*explicit as u64) > cur {
                        self.tables
                            .ensure_next_row_id(&stmt.table.name, *explicit as u64)?;
                    }
                }
            }
            rows_inserted += 1;
        }

        if rows_inserted > 0 {
            self.persist_table(&stmt.table.name)?;
            self.notify(|obs| obs.on_rows_inserted(&stmt.table.name, rows_inserted));
        }
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected: rows_inserted,
            stats: ExecutionStats::new(0, rows_inserted, elapsed),
        })
    }

    fn execute_select(&self, mut stmt: SelectStatement, start: &Instant) -> Result<SQLResult> {
        let _guard = self.engine_lock.read();
        if !self.tables.table_exists(&stmt.from.name) {
            return Err(SQLError::TableNotFound(stmt.from.name.clone()));
        }
        for join in &stmt.joins {
            if !self.tables.table_exists(&join.right.name) {
                return Err(SQLError::TableNotFound(join.right.name.clone()));
            }
        }

        let schema = self.combined_from_schema(&stmt.from, &stmt.joins)?;

        // Expand wildcards
        stmt.select_list = expand_wildcards(&stmt.select_list, &schema);

        // Extract column names from select list
        let col_names: Vec<String> = stmt
            .select_list
            .iter()
            .map(|item| match item {
                SelectItem::Expr { expr, alias } => {
                    if let Some(a) = alias {
                        a.clone()
                    } else {
                        match expr {
                            Expr::Column(name) => name.clone(),
                            Expr::Literal(lit) => format!("{:?}", lit),
                            Expr::Function { name, .. } => name.clone(),
                            _ => format!("{:?}", expr),
                        }
                    }
                }
                SelectItem::Wildcard => unreachable!("wildcards expanded"),
            })
            .collect();

        // Bind and type check
        let binder = Binder::new();
        let _bound = binder.bind(&stmt, &schema)?;

        // Create logical plan
        let planner = LogicalPlanner::new();
        let plan = planner.plan_select(stmt);

        // Build and execute - enforce sort limit via config
        let mut executor = build_executor(&plan, self.tables.clone(), self.config.max_batch_size)?;
        executor.open()?;

        let mut rows: Vec<Row> = Vec::new();
        while let Some(row) = executor.next()? {
            rows.push(row);
        }
        executor.close()?;

        // Apply HAVING after aggregation if present

        let batch = if col_names.is_empty() {
            rows_to_record_batch(&rows)
        } else {
            rows_to_record_batch_with_names(&rows, Some(&col_names))
        };
        let num_rows = batch.num_rows;

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Query {
            batches: vec![batch],
            stats: ExecutionStats::new(rows.len() as u64, num_rows as u64, elapsed),
        })
    }

    fn execute_update(&self, stmt: UpdateStatement, start: &Instant) -> Result<SQLResult> {
        let _guard = self.engine_lock.write();
        let schema = self.tables.get_schema(&stmt.table.name)?;
        let mut rows = self.tables.scan_rows(&stmt.table.name)?;
        let mut rows_affected = 0u64;

        for row in &mut rows {
            if let Some(ref predicate) = stmt.where_clause {
                let result = evaluate_expr(predicate, &row.values, &schema)?;
                if result != LiteralValue::Boolean(true) {
                    continue;
                }
            }

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

        // Atomic in-place update without dropping table (prevents concurrent SELECT TableNotFound)
        self.tables.replace_rows(&stmt.table.name, rows)?;
        self.persist_table(&stmt.table.name)?;
        if rows_affected > 0 {
            self.notify(|obs| obs.on_rows_updated(&stmt.table.name, rows_affected));
        }

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected,
            stats: ExecutionStats::new(0, rows_affected, elapsed),
        })
    }

    fn execute_delete(&self, stmt: DeleteStatement, start: &Instant) -> Result<SQLResult> {
        let _guard = self.engine_lock.write();
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

        self.tables.replace_rows(&stmt.table.name, kept_rows)?;
        self.persist_table(&stmt.table.name)?;
        if rows_affected > 0 {
            self.notify(|obs| obs.on_rows_deleted(&stmt.table.name, rows_affected));
        }

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(SQLResult::Exec {
            rows_affected,
            stats: ExecutionStats::new(0, rows_affected, elapsed),
        })
    }

    /// Concatenate schemas of all tables in the FROM and JOIN clauses into a
    /// single combined schema, reindexing column ordinals sequentially.
    fn combined_from_schema(&self, from: &TableRef, joins: &[Join]) -> Result<Schema> {
        let mut schema = self.tables.get_schema(&from.name)?;
        for join in joins {
            let right = self.tables.get_schema(&join.right.name)?;
            let offset = schema.len();
            for (i, col) in right.columns.iter().enumerate() {
                let mut c = col.clone();
                c.ordinal = offset + i;
                schema.columns.push(c);
            }
        }
        Ok(schema)
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
    rows_to_record_batch_with_names(rows, None)
}

fn rows_to_record_batch_with_names(rows: &[Row], column_names: Option<&[String]>) -> RecordBatch {
    if rows.is_empty() {
        let names = column_names.unwrap_or(&[]).to_vec();
        return RecordBatch::with_names(vec![], 0, names);
    }
    let num_cols = rows[0].values.len();
    let num_rows = rows.len();

    let names: Vec<String> = match column_names {
        Some(n) if n.len() == num_cols => n.to_vec(),
        _ => (0..num_cols).map(|i| format!("col_{}", i)).collect(),
    };

    let mut col_types: Vec<Option<SQLType>> = vec![None; num_cols];
    for row in rows {
        for (i, val) in row.values.iter().enumerate() {
            if col_types[i].is_none()
                && let Some(v) = val
            {
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
            let opt_val = val.clone().and_then(|v| {
                if matches!(v, LiteralValue::Null) {
                    None
                } else {
                    Some(v)
                }
            });
            push_value_to_column(&mut columns[i], opt_val);
        }
    }

    RecordBatch::with_names(columns, num_rows, names)
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
