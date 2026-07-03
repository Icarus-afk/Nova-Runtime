use crate::ast::{Expr, SelectItem, SelectStatement};
use crate::error::{Result, SQLError};
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct BoundColumn {
    pub name: String,
    pub ordinal: usize,
    pub table_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BoundSelect {
    pub select_list: Vec<BoundSelectItem>,
    pub columns: Vec<BoundColumn>,
}

#[derive(Debug, Clone)]
pub enum BoundSelectItem {
    Expr {
        expr: Expr,
        alias: Option<String>,
        source_index: Option<usize>,
    },
    Wildcard,
}

pub struct Binder;

impl Binder {
    pub fn new() -> Self {
        Binder
    }

    pub fn bind(
        &self,
        stmt: &SelectStatement,
        schema: &Schema,
    ) -> Result<BoundSelect> {
        let mut columns = Vec::new();
        let mut select_list = Vec::new();

        for item in &stmt.select_list {
            match item {
                SelectItem::Wildcard => {
                    for col in &schema.columns {
                        columns.push(BoundColumn {
                            name: col.name.clone(),
                            ordinal: col.ordinal,
                            table_name: None,
                        });
                    }
                    select_list.push(BoundSelectItem::Wildcard);
                }
                SelectItem::Expr { expr, alias } => {
                    self.resolve_expr(expr, schema)?;
                    let source_index = match expr {
                        Expr::Column(name) => schema.find_index(name),
                        _ => None,
                    };
                    select_list.push(BoundSelectItem::Expr {
                        expr: expr.clone(),
                        alias: alias.clone(),
                        source_index,
                    });
                    if let Some(col_name) = alias.clone().or_else(|| match expr {
                        Expr::Column(name) => Some(name.clone()),
                        _ => None,
                    }) {
                        columns.push(BoundColumn {
                            name: col_name,
                            ordinal: columns.len(),
                            table_name: None,
                        });
                    }
                }
            }
        }

        Ok(BoundSelect {
            select_list,
            columns,
        })
    }

    fn resolve_expr(&self, expr: &Expr, schema: &Schema) -> Result<()> {
        match expr {
            Expr::Column(name) => {
                if schema.find_column(name).is_none() {
                    return Err(SQLError::ColumnNotFound(name.clone()));
                }
            }
            Expr::Literal(_) => {}
            Expr::BinaryOp { left, right, .. } => {
                self.resolve_expr(left, schema)?;
                self.resolve_expr(right, schema)?;
            }
            Expr::UnaryOp { expr, .. } => {
                self.resolve_expr(expr, schema)?;
            }
            Expr::Function { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg, schema)?;
                }
            }
            Expr::IsNull(expr) | Expr::IsNotNull(expr) => {
                self.resolve_expr(expr, schema)?;
            }
            Expr::In { expr, list } => {
                self.resolve_expr(expr, schema)?;
                for e in list {
                    self.resolve_expr(e, schema)?;
                }
            }
            Expr::Between { expr, low, high } => {
                self.resolve_expr(expr, schema)?;
                self.resolve_expr(low, schema)?;
                self.resolve_expr(high, schema)?;
            }
            Expr::Like { expr, pattern } => {
                self.resolve_expr(expr, schema)?;
                self.resolve_expr(pattern, schema)?;
            }
        }
        Ok(())
    }
}
