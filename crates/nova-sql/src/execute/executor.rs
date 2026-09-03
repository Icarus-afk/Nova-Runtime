use crate::ast::{Expr, SQLType};
use crate::error::Result;
use crate::execute::iterators::{
    AggregateExecutor, DedupExecutor, Executor, FilterExecutor, JoinExecutor, LimitExecutor,
    ProjectionExecutor, ScanExecutor, SortExecutor,
};
use crate::execute::table_store::TableStore;
use crate::execute::table_store::TableStoreRef;
use crate::plan::logical::LogicalNode;
use crate::schema::{ColumnInfo, Schema};

pub fn build_executor(
    plan: &LogicalNode,
    tables: TableStoreRef,
    max_sort_rows: usize,
) -> Result<Box<dyn Executor>> {
    match plan {
        LogicalNode::Scan {
            table_name,
            alias: _,
        } => {
            let schema = tables.get_schema(table_name)?;
            Ok(Box::new(ScanExecutor::new(
                tables.clone(),
                table_name.clone(),
                schema,
            )))
        }
        LogicalNode::Selection { input, predicate } => {
            let input_exec = build_executor(input, tables.clone(), max_sort_rows)?;
            let schema = resolve_schema(input, tables.as_ref())?;
            Ok(Box::new(FilterExecutor::new(
                input_exec,
                predicate.clone(),
                schema,
            )))
        }
        LogicalNode::Projection { input, exprs } => {
            let input_exec = build_executor(input, tables.clone(), max_sort_rows)?;
            let in_schema = resolve_schema(input, tables.as_ref())?;
            let out_schema = projection_schema(exprs, &in_schema);
            Ok(Box::new(ProjectionExecutor::new_with_output(
                input_exec,
                exprs.clone(),
                in_schema,
                out_schema,
            )))
        }
        LogicalNode::Aggregate {
            input,
            exprs,
            group_by,
            having,
        } => {
            let input_exec = build_executor(input, tables.clone(), max_sort_rows)?;
            let schema = resolve_schema(input, tables.as_ref())?;
            Ok(Box::new(AggregateExecutor::new(
                input_exec,
                exprs.clone(),
                group_by.clone(),
                having.clone(),
                schema,
            )))
        }
        LogicalNode::Sort { input, order_by } => {
            let input_exec = build_executor(input, tables.clone(), max_sort_rows)?;
            // Sort operates on the projected/aggregated output, so resolve the
            // output schema (aliases + ordinals) rather than the raw table.
            let schema = resolve_output_schema(input, tables.as_ref())?;
            Ok(Box::new(SortExecutor::with_limit(
                input_exec,
                order_by.clone(),
                schema,
                max_sort_rows,
            )))
        }
        LogicalNode::Limit {
            input,
            limit,
            offset,
        } => {
            let input_exec = build_executor(input, tables.clone(), max_sort_rows)?;
            Ok(Box::new(LimitExecutor::new(input_exec, *limit, *offset)))
        }
        LogicalNode::Dedup { input } => {
            let input_exec = build_executor(input, tables.clone(), max_sort_rows)?;
            Ok(Box::new(DedupExecutor::new(input_exec)))
        }
        LogicalNode::Join { left, right, on } => {
            let left_exec = build_executor(left, tables.clone(), max_sort_rows)?;
            let right_exec = build_executor(right, tables.clone(), max_sort_rows)?;
            let left_schema = resolve_schema(left, tables.as_ref())?;
            let right_schema = resolve_schema(right, tables.as_ref())?;
            let schema = combine_schemas(&left_schema, &right_schema);
            Ok(Box::new(JoinExecutor::new(
                left_exec,
                right_exec,
                on.clone(),
                schema,
            )))
        }
    }
}

#[allow(clippy::explicit_counter_loop)]
fn combine_schemas(left: &Schema, right: &Schema) -> Schema {
    let mut columns = left.columns.clone();
    let mut offset = columns.len();
    for col in &right.columns {
        let mut c = col.clone();
        c.ordinal = offset;
        offset += 1;
        columns.push(c);
    }
    Schema::new(columns)
}

/// Output schema for a projection/aggregate: one column per select expr,
/// named by alias (or expression-derived name), so downstream Sort can
/// resolve aliases and ordinals against the result set.
fn projection_schema(exprs: &[(Expr, Option<String>)], _input: &Schema) -> Schema {
    let columns = exprs
        .iter()
        .enumerate()
        .map(|(ordinal, (expr, alias))| {
            let name = alias
                .clone()
                .unwrap_or_else(|| expr_name(expr).unwrap_or_else(|| format!("col_{}", ordinal)));
            ColumnInfo {
                name,
                sql_type: SQLType::Text,
                nullable: true,
                default: None,
                ordinal,
                unique: false,
                is_primary_key: false,
                auto_increment: false,
            }
        })
        .collect();
    Schema::new(columns)
}

fn expr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Column(c) => Some(c.clone()),
        Expr::Function { name, .. } => Some(name.clone()),
        Expr::Literal(lit) => Some(format!("{:?}", lit)),
        _ => None,
    }
}

fn resolve_schema(node: &LogicalNode, store: &TableStore) -> Result<Schema> {
    match node {
        LogicalNode::Scan { table_name, .. } => store.get_schema(table_name),
        LogicalNode::Join { left, right, .. } => {
            let left_schema = resolve_schema(left, store)?;
            let right_schema = resolve_schema(right, store)?;
            Ok(combine_schemas(&left_schema, &right_schema))
        }
        LogicalNode::Projection { input, .. }
        | LogicalNode::Selection { input, .. }
        | LogicalNode::Sort { input, .. }
        | LogicalNode::Limit { input, .. }
        | LogicalNode::Aggregate { input, .. }
        | LogicalNode::Dedup { input } => resolve_schema(input, store),
    }
}

/// Resolve the schema of rows produced by `node` — for projection/aggregate
/// this is the output (aliased) schema used by downstream Sort.
fn resolve_output_schema(node: &LogicalNode, store: &TableStore) -> Result<Schema> {
    match node {
        LogicalNode::Projection { input, exprs } => {
            let in_schema = resolve_schema(input, store)?;
            Ok(projection_schema(exprs, &in_schema))
        }
        LogicalNode::Aggregate { input, exprs, .. } => {
            let in_schema = resolve_schema(input, store)?;
            Ok(projection_schema(exprs, &in_schema))
        }
        LogicalNode::Selection { input, .. }
        | LogicalNode::Sort { input, .. }
        | LogicalNode::Limit { input, .. }
        | LogicalNode::Dedup { input } => resolve_output_schema(input, store),
        LogicalNode::Join { left, right, .. } => {
            let left_schema = resolve_schema(left, store)?;
            let right_schema = resolve_schema(right, store)?;
            Ok(combine_schemas(&left_schema, &right_schema))
        }
        LogicalNode::Scan { table_name, .. } => store.get_schema(table_name),
    }
}
