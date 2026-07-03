use crate::ast::{Expr, OrderByExpr};

#[derive(Debug, Clone)]
pub enum LogicalNode {
    Scan {
        table_name: String,
        alias: Option<String>,
    },
    Projection {
        input: Box<LogicalNode>,
        exprs: Vec<(Expr, Option<String>)>,
    },
    Selection {
        input: Box<LogicalNode>,
        predicate: Expr,
    },
    Sort {
        input: Box<LogicalNode>,
        order_by: Vec<OrderByExpr>,
    },
    Limit {
        input: Box<LogicalNode>,
        limit: usize,
        offset: usize,
    },
    Aggregate {
        input: Box<LogicalNode>,
        exprs: Vec<(Expr, Option<String>)>,
    },
    Dedup {
        input: Box<LogicalNode>,
    },
}
