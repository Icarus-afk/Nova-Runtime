use crate::error::Result;
use crate::execute::iterators::{
    AggregateExecutor, DedupExecutor, Executor, FilterExecutor, LimitExecutor, ProjectionExecutor,
    ScanExecutor, SortExecutor,
};
use crate::execute::table_store::TableStore;
use crate::execute::table_store::TableStoreRef;
use crate::plan::logical::LogicalNode;
use crate::schema::Schema;

pub fn build_executor(
    plan: &LogicalNode,
    tables: TableStoreRef,
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
            let input_exec = build_executor(input, tables.clone())?;
            let schema = resolve_schema(input, tables.as_ref())?;
            Ok(Box::new(FilterExecutor::new(input_exec, predicate.clone(), schema)))
        }
        LogicalNode::Projection { input, exprs } => {
            let input_exec = build_executor(input, tables.clone())?;
            let schema = resolve_schema(input, tables.as_ref())?;
            Ok(Box::new(ProjectionExecutor::new(
                input_exec,
                exprs.clone(),
                schema,
            )))
        }
        LogicalNode::Aggregate { input, exprs } => {
            let input_exec = build_executor(input, tables.clone())?;
            let schema = resolve_schema(input, tables.as_ref())?;
            Ok(Box::new(AggregateExecutor::new(
                input_exec,
                exprs.clone(),
                schema,
            )))
        }
        LogicalNode::Sort { input, order_by } => {
            let input_exec = build_executor(input, tables.clone())?;
            let schema = resolve_schema(input, tables.as_ref())?;
            Ok(Box::new(SortExecutor::new(
                input_exec,
                order_by.clone(),
                schema,
            )))
        }
        LogicalNode::Limit { input, limit, offset } => {
            let input_exec = build_executor(input, tables.clone())?;
            Ok(Box::new(LimitExecutor::new(input_exec, *limit, *offset)))
        }
        LogicalNode::Dedup { input } => {
            let input_exec = build_executor(input, tables.clone())?;
            Ok(Box::new(DedupExecutor::new(input_exec)))
        }
    }
}

fn resolve_schema(node: &LogicalNode, store: &TableStore) -> Result<Schema> {
    match node {
        LogicalNode::Scan { table_name, .. } => store.get_schema(table_name),
        LogicalNode::Projection { input, .. }
        | LogicalNode::Selection { input, .. }
        | LogicalNode::Sort { input, .. }
        | LogicalNode::Limit { input, .. }
        | LogicalNode::Aggregate { input, .. }
        | LogicalNode::Dedup { input } => resolve_schema(input, store),
    }
}
