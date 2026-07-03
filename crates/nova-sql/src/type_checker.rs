use crate::ast::{BinaryOperator, Expr, LiteralValue, SQLType, UnaryOperator};
use crate::error::{Result, SQLError};
use crate::schema::Schema;

pub struct TypeChecker;

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker
    }

    pub fn check_types(&self, expr: &Expr, schema: &Schema) -> Result<SQLType> {
        match expr {
            Expr::Column(name) => {
                let col = schema
                    .find_column(name)
                    .ok_or_else(|| SQLError::ColumnNotFound(name.clone()))?;
                Ok(col.sql_type.clone())
            }
            Expr::Literal(val) => Ok(literal_type(val)),
            Expr::BinaryOp { left, op, right } => {
                let left_type = self.check_types(left, schema)?;
                let right_type = self.check_types(right, schema)?;
                self.check_binary_op(*op, &left_type, &right_type)
            }
            Expr::UnaryOp { op, expr } => {
                let inner = self.check_types(expr, schema)?;
                self.check_unary_op(*op, &inner)
            }
            Expr::Function { name, args } => {
                self.check_function(name, args, schema)
            }
            Expr::IsNull(_) | Expr::IsNotNull(_) => Ok(SQLType::Boolean),
            Expr::In { expr, list } => {
                let expr_type = self.check_types(expr, schema)?;
                for item in list {
                    let item_type = self.check_types(item, schema)?;
                    self.unify_types(&expr_type, &item_type)?;
                }
                Ok(SQLType::Boolean)
            }
            Expr::Between { expr, low, high } => {
                let et = self.check_types(expr, schema)?;
                let lt = self.check_types(low, schema)?;
                let ht = self.check_types(high, schema)?;
                self.unify_types(&et, &lt)?;
                self.unify_types(&et, &ht)?;
                Ok(SQLType::Boolean)
            }
            Expr::Like { expr, pattern } | Expr::ILike { expr, pattern } => {
                self.check_types(expr, schema)?;
                self.check_types(pattern, schema)?;
                Ok(SQLType::Boolean)
            }
            Expr::Case { whens, else_val } => {
                let mut result_type = SQLType::Null;
                for (_, result) in whens {
                    let rt = self.check_types(result, schema)?;
                    result_type = self.unify_types(&result_type, &rt)?;
                }
                if let Some(e) = else_val {
                    let rt = self.check_types(e, schema)?;
                    result_type = self.unify_types(&result_type, &rt)?;
                }
                Ok(result_type)
            }
            Expr::Cast { expr, target_type } => {
                self.check_types(expr, schema)?;
                Ok(target_type.clone())
            }
        }
    }

    fn check_binary_op(
        &self,
        op: BinaryOperator,
        left: &SQLType,
        right: &SQLType,
    ) -> Result<SQLType> {
        match op {
            BinaryOperator::And | BinaryOperator::Or => {
                if *left != SQLType::Boolean {
                    return Err(SQLError::TypeMismatch {
                        expected: "BOOLEAN".to_string(),
                        actual: format!("{:?}", left),
                    });
                }
                if *right != SQLType::Boolean {
                    return Err(SQLError::TypeMismatch {
                        expected: "BOOLEAN".to_string(),
                        actual: format!("{:?}", right),
                    });
                }
                Ok(SQLType::Boolean)
            }
            BinaryOperator::Eq | BinaryOperator::NotEq => {
                let unified = self.unify_types(left, right)?;
                Ok(unified)
            }
            BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::Concat => {
                let unified = self.unify_types(left, right)?;
                match unified {
                    SQLType::Null | SQLType::Boolean => {
                        Ok(SQLType::Boolean)
                    }
                    SQLType::Integer | SQLType::Float => {
                        Ok(SQLType::Boolean)
                    }
                    SQLType::Text => {
                        if op == BinaryOperator::Concat {
                            Ok(SQLType::Text)
                        } else {
                            Ok(SQLType::Boolean)
                        }
                    }
                }
            }
            BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => {
                let unified = self.unify_types(left, right)?;
                match unified {
                    SQLType::Null => Ok(SQLType::Null),
                    SQLType::Integer => Ok(SQLType::Integer),
                    SQLType::Float => Ok(SQLType::Float),
                    SQLType::Text => Ok(SQLType::Text),
                    SQLType::Boolean => Err(SQLError::TypeMismatch {
                        expected: "numeric".to_string(),
                        actual: "BOOLEAN".to_string(),
                    }),
                }
            }
        }
    }

    fn check_unary_op(&self, op: UnaryOperator, inner: &SQLType) -> Result<SQLType> {
        match op {
            UnaryOperator::Neg => match inner {
                SQLType::Integer | SQLType::Float | SQLType::Null => Ok(inner.clone()),
                other => Err(SQLError::TypeMismatch {
                    expected: "numeric".to_string(),
                    actual: format!("{:?}", other),
                }),
            },
            UnaryOperator::Not => match inner {
                SQLType::Boolean | SQLType::Null => Ok(SQLType::Boolean),
                other => Err(SQLError::TypeMismatch {
                    expected: "BOOLEAN".to_string(),
                    actual: format!("{:?}", other),
                }),
            },
        }
    }

    fn check_function(
        &self,
        name: &str,
        args: &[Expr],
        schema: &Schema,
    ) -> Result<SQLType> {
        let lower = name.to_lowercase();
        match lower.as_str() {
            "count" => Ok(SQLType::Integer),
            "sum" | "avg" => {
                if args.is_empty() {
                    return Err(SQLError::syntax(
                        format!("{} requires at least one argument", name),
                    ));
                }
                let arg_type = self.check_types(&args[0], schema)?;
                match arg_type {
                    SQLType::Integer | SQLType::Float => {
                        if lower == "avg" {
                            Ok(SQLType::Float)
                        } else {
                            Ok(arg_type)
                        }
                    }
                    _ => Err(SQLError::TypeMismatch {
                        expected: "numeric".to_string(),
                        actual: format!("{:?}", arg_type),
                    }),
                }
            }
            "min" | "max" => {
                if args.is_empty() {
                    return Err(SQLError::syntax(
                        format!("{} requires at least one argument", name),
                    ));
                }
                let arg_type = self.check_types(&args[0], schema)?;
                Ok(arg_type)
            }
            _ => {
                Ok(SQLType::Null)
            }
        }
    }

    fn unify_types(&self, a: &SQLType, b: &SQLType) -> Result<SQLType> {
        if *a == SQLType::Null {
            return Ok(b.clone());
        }
        if *b == SQLType::Null {
            return Ok(a.clone());
        }
        if *a == *b {
            return Ok(a.clone());
        }
        match (a, b) {
            (SQLType::Integer, SQLType::Float) => Ok(SQLType::Float),
            (SQLType::Float, SQLType::Integer) => Ok(SQLType::Float),
            (SQLType::Integer, SQLType::Text) => Ok(SQLType::Integer),
            (SQLType::Text, SQLType::Integer) => Ok(SQLType::Integer),
            (SQLType::Float, SQLType::Text) => Ok(SQLType::Float),
            (SQLType::Text, SQLType::Float) => Ok(SQLType::Float),
            _ => Err(SQLError::TypeMismatch {
                expected: format!("{:?}", a),
                actual: format!("{:?}", b),
            }),
        }
    }

    pub fn coerce_value(val: &LiteralValue, target: &SQLType) -> Result<LiteralValue> {
        match (val, target) {
            (LiteralValue::Null, _) => Ok(LiteralValue::Null),
            (v, t) if literal_type(v) == *t => Ok(v.clone()),
            (LiteralValue::Integer(i), SQLType::Float) => {
                Ok(LiteralValue::Float(*i as f64))
            }
            (LiteralValue::Float(f), SQLType::Integer) => {
                Ok(LiteralValue::Integer(*f as i64))
            }
            (LiteralValue::String(s), SQLType::Integer) => {
                let val: i64 = s
                    .parse()
                    .map_err(|_| SQLError::TypeMismatch {
                        expected: "INTEGER".to_string(),
                        actual: format!("string: {}", s),
                    })?;
                Ok(LiteralValue::Integer(val))
            }
            (LiteralValue::String(s), SQLType::Float) => {
                let val: f64 = s
                    .parse()
                    .map_err(|_| SQLError::TypeMismatch {
                        expected: "FLOAT".to_string(),
                        actual: format!("string: {}", s),
                    })?;
                Ok(LiteralValue::Float(val))
            }
            (LiteralValue::String(s), SQLType::Text) => {
                Ok(LiteralValue::String(s.clone()))
            }
            (LiteralValue::Integer(i), SQLType::Text) => {
                Ok(LiteralValue::String(i.to_string()))
            }
            (LiteralValue::Float(f), SQLType::Text) => {
                Ok(LiteralValue::String(f.to_string()))
            }
            (LiteralValue::Boolean(b), SQLType::Text) => {
                Ok(LiteralValue::String(b.to_string()))
            }
            (LiteralValue::String(s), SQLType::Boolean) => {
                let val = match s.to_lowercase().as_str() {
                    "true" | "1" | "yes" => true,
                    "false" | "0" | "no" => false,
                    _ => {
                        return Err(SQLError::TypeMismatch {
                            expected: "BOOLEAN".to_string(),
                            actual: format!("string: {}", s),
                        })
                    }
                };
                Ok(LiteralValue::Boolean(val))
            }
            (LiteralValue::Integer(i), SQLType::Boolean) => {
                Ok(LiteralValue::Boolean(*i != 0))
            }
            _ => Err(SQLError::TypeMismatch {
                expected: format!("{:?}", target),
                actual: format!("{:?}", literal_type(val)),
            }),
        }
    }
}

pub fn literal_type(val: &LiteralValue) -> SQLType {
    match val {
        LiteralValue::Null => SQLType::Null,
        LiteralValue::Boolean(_) => SQLType::Boolean,
        LiteralValue::Integer(_) => SQLType::Integer,
        LiteralValue::Float(_) => SQLType::Float,
        LiteralValue::String(_) => SQLType::Text,
    }
}
