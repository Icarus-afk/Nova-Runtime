pub mod executor;
pub mod iterators;
pub mod table_store;

use crate::ast::{
    BinaryOperator, Expr, LiteralValue, UnaryOperator,
};

pub fn contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Function { name, .. } => {
            matches!(name.to_lowercase().as_str(), "count" | "sum" | "avg" | "min" | "max")
        }
        Expr::BinaryOp { left, right, .. } => contains_aggregate(left) || contains_aggregate(right),
        Expr::UnaryOp { expr, .. } => contains_aggregate(expr),
        _ => false,
    }
}
use crate::error::{Result, SQLError};
use crate::schema::Schema;

pub fn evaluate_expr(
    expr: &Expr,
    row: &[Option<LiteralValue>],
    schema: &Schema,
) -> Result<LiteralValue> {
    match expr {
        Expr::Column(name) => {
            let idx = schema
                .find_index(name)
                .ok_or_else(|| SQLError::ColumnNotFound(name.clone()))?;
            Ok(row[idx].clone().unwrap_or(LiteralValue::Null))
        }
        Expr::Literal(val) => Ok(val.clone()),
        Expr::BinaryOp { left, op, right } => {
            let left_val = evaluate_expr(left, row, schema)?;
            let right_val = evaluate_expr(right, row, schema)?;
            eval_binary_op(*op, &left_val, &right_val)
        }
        Expr::UnaryOp { op, expr } => {
            let val = evaluate_expr(expr, row, schema)?;
            eval_unary_op(*op, &val)
        }
        Expr::Function { name, args } => {
            let arg_vals: Result<Vec<LiteralValue>> = args
                .iter()
                .map(|a| evaluate_expr(a, row, schema))
                .collect();
            eval_function(name, &arg_vals?)
        }
        Expr::IsNull(expr) => {
            let val = evaluate_expr(expr, row, schema)?;
            Ok(LiteralValue::Boolean(matches!(val, LiteralValue::Null)))
        }
        Expr::IsNotNull(expr) => {
            let val = evaluate_expr(expr, row, schema)?;
            Ok(LiteralValue::Boolean(!matches!(val, LiteralValue::Null)))
        }
        Expr::In { expr, list } => {
            let val = evaluate_expr(expr, row, schema)?;
            for item in list {
                let item_val = evaluate_expr(item, row, schema)?;
                if val == item_val {
                    return Ok(LiteralValue::Boolean(true));
                }
            }
            Ok(LiteralValue::Boolean(false))
        }
        Expr::Between { expr, low, high } => {
            let val = evaluate_expr(expr, row, schema)?;
            let low_val = evaluate_expr(low, row, schema)?;
            let high_val = evaluate_expr(high, row, schema)?;
            let ge = eval_binary_op(BinaryOperator::GtEq, &val, &low_val)?;
            let le = eval_binary_op(BinaryOperator::LtEq, &val, &high_val)?;
            Ok(LiteralValue::Boolean(
                ge == LiteralValue::Boolean(true)
                    && le == LiteralValue::Boolean(true),
            ))
        }
        Expr::Like { expr, pattern } => {
            let val = evaluate_expr(expr, row, schema)?;
            let pat = evaluate_expr(pattern, row, schema)?;
            eval_like(&val, &pat)
        }
    }
}

pub fn eval_binary_op(
    op: BinaryOperator,
    left: &LiteralValue,
    right: &LiteralValue,
) -> Result<LiteralValue> {
    match op {
        BinaryOperator::And => {
            let l = coerce_to_bool(left)?;
            let r = coerce_to_bool(right)?;
            Ok(LiteralValue::Boolean(l && r))
        }
        BinaryOperator::Or => {
            let l = coerce_to_bool(left)?;
            let r = coerce_to_bool(right)?;
            Ok(LiteralValue::Boolean(l || r))
        }
        BinaryOperator::Eq => {
            if *left == LiteralValue::Null || *right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            Ok(LiteralValue::Boolean(left == right))
        }
        BinaryOperator::NotEq => {
            if *left == LiteralValue::Null || *right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            Ok(LiteralValue::Boolean(left != right))
        }
        BinaryOperator::Lt
        | BinaryOperator::LtEq
        | BinaryOperator::Gt
        | BinaryOperator::GtEq => {
            if *left == LiteralValue::Null || *right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            let coerced = coerce_pair(left, right)?;
            let result = match (coerced.0, coerced.1) {
                (LiteralValue::Integer(a), LiteralValue::Integer(b)) => match op {
                    BinaryOperator::Lt => a < b,
                    BinaryOperator::LtEq => a <= b,
                    BinaryOperator::Gt => a > b,
                    BinaryOperator::GtEq => a >= b,
                    _ => unreachable!(),
                },
                (LiteralValue::Float(a), LiteralValue::Float(b)) => match op {
                    BinaryOperator::Lt => a < b,
                    BinaryOperator::LtEq => a <= b,
                    BinaryOperator::Gt => a > b,
                    BinaryOperator::GtEq => a >= b,
                    _ => unreachable!(),
                },
                (LiteralValue::String(a), LiteralValue::String(b)) => match op {
                    BinaryOperator::Lt => a < b,
                    BinaryOperator::LtEq => a <= b,
                    BinaryOperator::Gt => a > b,
                    BinaryOperator::GtEq => a >= b,
                    _ => unreachable!(),
                },
                _ => {
                    return Err(SQLError::TypeMismatch {
                        expected: "comparable types".to_string(),
                        actual: format!("{:?} vs {:?}", left, right),
                    })
                }
            };
            Ok(LiteralValue::Boolean(result))
        }
        BinaryOperator::Plus => {
            let coerced = coerce_numeric_pair(left, right)?;
            match coerced {
                (LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                    Ok(LiteralValue::Integer(a + b))
                }
                (LiteralValue::Float(a), LiteralValue::Float(b)) => {
                    Ok(LiteralValue::Float(a + b))
                }
                _ => Ok(LiteralValue::Null),
            }
        }
        BinaryOperator::Minus => {
            let coerced = coerce_numeric_pair(left, right)?;
            match coerced {
                (LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                    Ok(LiteralValue::Integer(a - b))
                }
                (LiteralValue::Float(a), LiteralValue::Float(b)) => {
                    Ok(LiteralValue::Float(a - b))
                }
                _ => Ok(LiteralValue::Null),
            }
        }
        BinaryOperator::Multiply => {
            let coerced = coerce_numeric_pair(left, right)?;
            match coerced {
                (LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                    Ok(LiteralValue::Integer(a * b))
                }
                (LiteralValue::Float(a), LiteralValue::Float(b)) => {
                    Ok(LiteralValue::Float(a * b))
                }
                _ => Ok(LiteralValue::Null),
            }
        }
        BinaryOperator::Divide => {
            let coerced = coerce_numeric_pair(left, right)?;
            match coerced {
                (LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                    if b == 0 {
                        return Err(SQLError::Internal("division by zero".to_string()));
                    }
                    Ok(LiteralValue::Integer(a / b))
                }
                (LiteralValue::Float(a), LiteralValue::Float(b)) => {
                    if b == 0.0 {
                        return Err(SQLError::Internal("division by zero".to_string()));
                    }
                    Ok(LiteralValue::Float(a / b))
                }
                _ => Ok(LiteralValue::Null),
            }
        }
        BinaryOperator::Modulo => {
            let coerced = coerce_numeric_pair(left, right)?;
            match coerced {
                (LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                    if b == 0 {
                        return Err(SQLError::Internal("division by zero".to_string()));
                    }
                    Ok(LiteralValue::Integer(a % b))
                }
                _ => Ok(LiteralValue::Null),
            }
        }
        BinaryOperator::Concat => {
            let l_str = value_to_string(left);
            let r_str = value_to_string(right);
            Ok(LiteralValue::String(format!("{}{}", l_str, r_str)))
        }
    }
}

fn eval_unary_op(op: UnaryOperator, val: &LiteralValue) -> Result<LiteralValue> {
    match op {
        UnaryOperator::Neg => match val {
            LiteralValue::Integer(i) => Ok(LiteralValue::Integer(-i)),
            LiteralValue::Float(f) => Ok(LiteralValue::Float(-f)),
            LiteralValue::Null => Ok(LiteralValue::Null),
            _ => Err(SQLError::TypeMismatch {
                expected: "numeric".to_string(),
                actual: format!("{:?}", val),
            }),
        },
        UnaryOperator::Not => match val {
            LiteralValue::Boolean(b) => Ok(LiteralValue::Boolean(!b)),
            LiteralValue::Null => Ok(LiteralValue::Null),
            _ => Err(SQLError::TypeMismatch {
                expected: "BOOLEAN".to_string(),
                actual: format!("{:?}", val),
            }),
        },
    }
}

fn eval_function(name: &str, args: &[LiteralValue]) -> Result<LiteralValue> {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "count" => {
            let non_null = args.iter().filter(|a| !matches!(a, LiteralValue::Null)).count();
            Ok(LiteralValue::Integer(non_null as i64))
        }
        "sum" => {
            let total: i64 = args
                .iter()
                .filter_map(|a| match a {
                    LiteralValue::Integer(i) => Some(*i),
                    _ => None,
                })
                .sum();
            Ok(LiteralValue::Integer(total))
        }
        "avg" => {
            let nums: Vec<f64> = args
                .iter()
                .filter_map(|a| match a {
                    LiteralValue::Integer(i) => Some(*i as f64),
                    LiteralValue::Float(f) => Some(*f),
                    _ => None,
                })
                .collect();
            if nums.is_empty() {
                return Ok(LiteralValue::Null);
            }
            let sum: f64 = nums.iter().sum();
            Ok(LiteralValue::Float(sum / nums.len() as f64))
        }
        "min" => {
            let nums: Vec<&LiteralValue> =
                args.iter().filter(|a| !matches!(a, LiteralValue::Null)).collect();
            if nums.is_empty() {
                return Ok(LiteralValue::Null);
            }
            let mut best = nums[0].clone();
            for v in &nums[1..] {
                let cmp = eval_binary_op(BinaryOperator::Lt, v, &best)?;
                if cmp == LiteralValue::Boolean(true) {
                    best = (*v).clone();
                }
            }
            Ok(best)
        }
        "max" => {
            let nums: Vec<&LiteralValue> =
                args.iter().filter(|a| !matches!(a, LiteralValue::Null)).collect();
            if nums.is_empty() {
                return Ok(LiteralValue::Null);
            }
            let mut best = nums[0].clone();
            for v in &nums[1..] {
                let cmp = eval_binary_op(BinaryOperator::Gt, v, &best)?;
                if cmp == LiteralValue::Boolean(true) {
                    best = (*v).clone();
                }
            }
            Ok(best)
        }
        _ => Ok(LiteralValue::Null),
    }
}

fn eval_like(val: &LiteralValue, pattern: &LiteralValue) -> Result<LiteralValue> {
    let s = match val {
        LiteralValue::String(s) => s.clone(),
        LiteralValue::Null => return Ok(LiteralValue::Null),
        _ => return Ok(LiteralValue::Boolean(false)),
    };
    let pat = match pattern {
        LiteralValue::String(p) => p.clone(),
        LiteralValue::Null => return Ok(LiteralValue::Null),
        _ => return Ok(LiteralValue::Boolean(false)),
    };
    // Convert SQL LIKE pattern to regex
    let regex_str = pat_to_regex(&pat);
    let re = regex::Regex::new(&regex_str)
        .map_err(|e| SQLError::Internal(format!("invalid LIKE pattern: {}", e)))?;
    Ok(LiteralValue::Boolean(re.is_match(&s)))
}

fn pat_to_regex(pat: &str) -> String {
    let mut result = String::from("^");
    let mut chars = pat.chars();
    while let Some(c) = chars.next() {
        match c {
            '%' => result.push_str(".*"),
            '_' => result.push('.'),
            // Escape regex special chars
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' => {
                result.push('\\');
                result.push(c);
            }
            c => result.push(c),
        }
    }
    result.push('$');
    result
}

fn coerce_to_bool(val: &LiteralValue) -> Result<bool> {
    match val {
        LiteralValue::Boolean(b) => Ok(*b),
        LiteralValue::Null => Ok(false),
        _ => Err(SQLError::TypeMismatch {
            expected: "BOOLEAN".to_string(),
            actual: format!("{:?}", val),
        }),
    }
}

fn coerce_pair(
    left: &LiteralValue,
    right: &LiteralValue,
) -> Result<(LiteralValue, LiteralValue)> {
    use LiteralValue::*;
    match (left, right) {
        (Integer(_), Integer(_)) => Ok((left.clone(), right.clone())),
        (Float(_), Float(_)) => Ok((left.clone(), right.clone())),
        (String(_), String(_)) => Ok((left.clone(), right.clone())),
        (Integer(i), Float(f)) => Ok((Float(*i as f64), Float(*f))),
        (Float(f), Integer(i)) => Ok((Float(*f), Float(*i as f64))),
        (Integer(_i), String(_)) => Ok((left.clone(), right.clone())),
        (String(_), Integer(_i)) => Ok((left.clone(), right.clone())),
        _ => Err(SQLError::TypeMismatch {
            expected: "matching types".to_string(),
            actual: format!("{:?} vs {:?}", left, right),
        }),
    }
}

fn coerce_numeric_pair(
    left: &LiteralValue,
    right: &LiteralValue,
) -> Result<(LiteralValue, LiteralValue)> {
    use LiteralValue::*;
    match (left, right) {
        (Integer(a), Integer(b)) => Ok((Integer(*a), Integer(*b))),
        (Integer(i), Float(f)) => Ok((Float(*i as f64), Float(*f))),
        (Float(f), Integer(i)) => Ok((Float(*f), Float(*i as f64))),
        (Float(_), Float(_)) => Ok((left.clone(), right.clone())),
        _ => Err(SQLError::TypeMismatch {
            expected: "numeric".to_string(),
            actual: format!("{:?} vs {:?}", left, right),
        }),
    }
}

fn value_to_string(val: &LiteralValue) -> String {
    match val {
        LiteralValue::Null => "NULL".to_string(),
        LiteralValue::Boolean(b) => b.to_string(),
        LiteralValue::Integer(i) => i.to_string(),
        LiteralValue::Float(f) => f.to_string(),
        LiteralValue::String(s) => s.clone(),
    }
}
