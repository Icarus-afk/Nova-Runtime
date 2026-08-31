use crate::ast::*;
use crate::execute::contains_aggregate;
use crate::plan::logical::LogicalNode;

pub struct LogicalPlanner;

impl LogicalPlanner {
    pub fn new() -> Self {
        LogicalPlanner
    }

    pub fn plan_select(&self, stmt: SelectStatement) -> LogicalNode {
        // Build the base relation: left-deep tree of joins.
        let mut node = LogicalNode::Scan {
            table_name: stmt.from.name.clone(),
            alias: stmt.from.alias.clone(),
        };

        for join in &stmt.joins {
            let right = LogicalNode::Scan {
                table_name: join.right.name.clone(),
                alias: join.right.alias.clone(),
            };
            node = LogicalNode::Join {
                left: Box::new(node),
                right: Box::new(right),
                on: join.on.clone(),
            };
        }

        if let Some(predicate) = stmt.where_clause {
            node = LogicalNode::Selection {
                input: Box::new(node),
                predicate,
            };
        }

        let exprs: Vec<(Expr, Option<String>)> = stmt
            .select_list
            .into_iter()
            .map(|item| match item {
                SelectItem::Expr { expr, alias } => (expr, alias),
                SelectItem::Wildcard => {
                    (Expr::Literal(LiteralValue::String("*".to_string())), None)
                }
            })
            .collect();

        let has_group_by = !stmt.group_by.is_empty();
        let has_aggregate =
            has_group_by || exprs.iter().any(|(e, _)| contains_aggregate(e));

        if has_aggregate {
            node = LogicalNode::Aggregate {
                input: Box::new(node),
                exprs,
                group_by: stmt.group_by,
                having: stmt.having,
            };
        } else {
            node = LogicalNode::Projection {
                input: Box::new(node),
                exprs,
            };
        }

        // ORDER BY must run on the projected (or aggregated) output so aliases
        // and ordinals resolve against the result set, not the raw table.
        if !stmt.order_by.is_empty() {
            node = LogicalNode::Sort {
                input: Box::new(node),
                order_by: stmt.order_by,
            };
        }

        if stmt.distinct {
            node = LogicalNode::Dedup {
                input: Box::new(node),
            };
        }

        let limit = stmt.limit.unwrap_or(usize::MAX);
        let offset = stmt.offset.unwrap_or(0);
        node = LogicalNode::Limit {
            input: Box::new(node),
            limit,
            offset,
        };

        node
    }
}
