use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::ast::{BinaryOperator, Expr, LiteralValue, OrderByExpr};
use crate::error::Result;
use crate::execute::contains_aggregate;
use crate::execute::evaluate_expr;
use crate::execute::table_store::{Row, TableStoreRef};
use crate::schema::Schema;

pub trait Executor {
    fn open(&mut self) -> Result<()>;
    fn next(&mut self) -> Result<Option<Row>>;
    fn close(&mut self) -> Result<()>;
}

pub struct ScanExecutor {
    tables: TableStoreRef,
    table_name: String,
    _schema: Schema,
    rows: Vec<Row>,
    index: usize,
}

impl ScanExecutor {
    pub fn new(tables: TableStoreRef, table_name: String, schema: Schema) -> Self {
        ScanExecutor {
            tables,
            table_name,
            _schema: schema,
            rows: Vec::new(),
            index: 0,
        }
    }
}

impl Executor for ScanExecutor {
    fn open(&mut self) -> Result<()> {
        self.rows = self.tables.scan_rows(&self.table_name)?;
        self.index = 0;
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Row>> {
        if self.index >= self.rows.len() {
            return Ok(None);
        }
        let row = self.rows[self.index].clone();
        self.index += 1;
        Ok(Some(row))
    }

    fn close(&mut self) -> Result<()> {
        self.rows.clear();
        self.index = 0;
        Ok(())
    }
}

pub struct FilterExecutor {
    input: Box<dyn Executor>,
    predicate: Expr,
    schema: Schema,
}

impl FilterExecutor {
    pub fn new(input: Box<dyn Executor>, predicate: Expr, schema: Schema) -> Self {
        FilterExecutor {
            input,
            predicate,
            schema,
        }
    }
}

impl Executor for FilterExecutor {
    fn open(&mut self) -> Result<()> {
        self.input.open()
    }

    fn next(&mut self) -> Result<Option<Row>> {
        loop {
            match self.input.next()? {
                None => return Ok(None),
                Some(row) => {
                    let result = evaluate_expr(&self.predicate, &row.values, &self.schema)?;
                    match result {
                        LiteralValue::Boolean(true) => return Ok(Some(row)),
                        _ => continue,
                    }
                }
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.input.close()
    }
}

pub struct ProjectionExecutor {
    input: Box<dyn Executor>,
    exprs: Vec<(Expr, Option<String>)>,
    schema: Schema,
    _output_schema: Schema,
}

impl ProjectionExecutor {
    pub fn new(
        input: Box<dyn Executor>,
        exprs: Vec<(Expr, Option<String>)>,
        schema: Schema,
    ) -> Self {
        ProjectionExecutor {
            input,
            exprs,
            schema,
            _output_schema: Schema::default(),
        }
    }

    pub fn new_with_output(
        input: Box<dyn Executor>,
        exprs: Vec<(Expr, Option<String>)>,
        schema: Schema,
        output_schema: Schema,
    ) -> Self {
        ProjectionExecutor {
            input,
            exprs,
            schema,
            _output_schema: output_schema,
        }
    }
}

impl Executor for ProjectionExecutor {
    fn open(&mut self) -> Result<()> {
        self.input.open()
    }

    fn next(&mut self) -> Result<Option<Row>> {
        match self.input.next()? {
            None => Ok(None),
            Some(row) => {
                let mut values = Vec::with_capacity(self.exprs.len());
                for (expr, _alias) in &self.exprs {
                    let val = evaluate_expr(expr, &row.values, &self.schema)?;
                    values.push(Some(val));
                }
                Ok(Some(Row::new(values)))
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.input.close()
    }
}

pub struct DedupExecutor {
    input: Box<dyn Executor>,
    seen: Vec<u64>,
}

impl DedupExecutor {
    pub fn new(input: Box<dyn Executor>) -> Self {
        DedupExecutor {
            input,
            seen: Vec::new(),
        }
    }

    fn row_hash(row: &Row) -> u64 {
        let mut hasher = DefaultHasher::new();
        for val in &row.values {
            match val {
                Some(LiteralValue::Null) => 0u64.hash(&mut hasher),
                Some(LiteralValue::Boolean(b)) => b.hash(&mut hasher),
                Some(LiteralValue::Integer(i)) => i.hash(&mut hasher),
                Some(LiteralValue::Float(f)) => f.to_bits().hash(&mut hasher),
                Some(LiteralValue::String(s)) => s.hash(&mut hasher),
                None => 1u64.hash(&mut hasher),
            }
        }
        hasher.finish()
    }
}

impl Executor for DedupExecutor {
    fn open(&mut self) -> Result<()> {
        self.input.open()?;
        self.seen.clear();
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Row>> {
        loop {
            match self.input.next()? {
                None => return Ok(None),
                Some(row) => {
                    let h = Self::row_hash(&row);
                    if self.seen.contains(&h) {
                        continue;
                    }
                    self.seen.push(h);
                    return Ok(Some(row));
                }
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.input.close()
    }
}

pub struct AggregateExecutor {
    input: Box<dyn Executor>,
    exprs: Vec<(Expr, Option<String>)>,
    group_by: Vec<Expr>,
    having: Option<Expr>,
    schema: Schema,
    results: Vec<Row>,
    index: usize,
}

impl AggregateExecutor {
    pub fn new(
        input: Box<dyn Executor>,
        exprs: Vec<(Expr, Option<String>)>,
        group_by: Vec<Expr>,
        having: Option<Expr>,
        schema: Schema,
    ) -> Self {
        AggregateExecutor {
            input,
            exprs,
            group_by,
            having,
            schema,
            results: Vec::new(),
            index: 0,
        }
    }
}

impl Executor for AggregateExecutor {
    fn open(&mut self) -> Result<()> {
        self.input.open()?;
        let mut input_rows = Vec::new();
        while let Some(row) = self.input.next()? {
            input_rows.push(row);
        }
        self.input.close()?;

        let schema = self.schema.clone();
        let exprs = self.exprs.clone();
        let group_by = self.group_by.clone();
        let having = self.having.clone();

        // Partition into groups when GROUP BY is present.
        let mut groups: Vec<(Vec<LiteralValue>, Vec<Row>)> = Vec::new();
        if group_by.is_empty() {
            if !input_rows.is_empty() {
                groups.push((Vec::new(), input_rows));
            }
        } else {
            for row in input_rows {
                let key = evaluate_group_key(&group_by, &row.values, &schema)?;
                match groups.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, rows)) => rows.push(row),
                    None => groups.push((key, vec![row])),
                }
            }
        }

        let mut results = Vec::with_capacity(groups.len());
        for (group_key, group_rows) in groups {
            let mut row_values = Vec::with_capacity(exprs.len());
            for (expr, _alias) in &exprs {
                let val = evaluate_grouped_expr(expr, &group_key, &group_rows, &schema)?;
                row_values.push(Some(val));
            }
            let agg_row = Row::new(row_values);
            if let Some(ref h) = having {
                // Evaluate HAVING as an aggregate-aware predicate over the
                // group's rows (the input schema), so aggregate functions in
                // the predicate can be computed on this group.
                let hv = evaluate_grouped_expr(h, &group_key, &group_rows, &schema)?;
                if hv == LiteralValue::Boolean(true) {
                    results.push(agg_row);
                }
            } else {
                results.push(agg_row);
            }
        }

        self.results = results;
        self.index = 0;
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Row>> {
        if self.index >= self.results.len() {
            return Ok(None);
        }
        let row = self.results[self.index].clone();
        self.index += 1;
        Ok(Some(row))
    }

    fn close(&mut self) -> Result<()> {
        self.results.clear();
        self.index = 0;
        Ok(())
    }
}

fn evaluate_group_key(
    group_by: &[Expr],
    row: &[Option<LiteralValue>],
    schema: &Schema,
) -> Result<Vec<LiteralValue>> {
    group_by
        .iter()
        .map(|e| evaluate_expr(e, row, schema))
        .collect()
}

/// Evaluate a select-list expression in an aggregate context.
/// - A plain column that matches a GROUP BY column yields the group's value.
/// - An aggregate function aggregates over the group's rows.
/// - Nested expressions propagate accordingly.
#[allow(clippy::only_used_in_recursion)]
fn evaluate_grouped_expr(
    expr: &Expr,
    group_key: &[LiteralValue],
    group_rows: &[Row],
    schema: &Schema,
) -> Result<LiteralValue> {
    match expr {
        Expr::Column(_) => {
            // Plain column in an aggregate context: take the representative
            // value from the first row of the group (matches GROUP BY columns).
            if !group_rows.is_empty() {
                return evaluate_expr(expr, &group_rows[0].values, schema);
            }
            Ok(LiteralValue::Null)
        }
        Expr::Function { .. } if contains_aggregate(expr) => {
            evaluate_aggregate_expr(expr, group_rows, schema)
        }
        Expr::BinaryOp { left, op, right } => {
            let l = evaluate_grouped_expr(left, group_key, group_rows, schema)?;
            let r = evaluate_grouped_expr(right, group_key, group_rows, schema)?;
            crate::execute::eval_binary_op(*op, &l, &r)
        }
        Expr::UnaryOp { op, expr: inner } => {
            let v = evaluate_grouped_expr(inner, group_key, group_rows, schema)?;
            crate::execute::eval_unary_op(*op, &v)
        }
        Expr::Cast {
            expr: inner,
            target_type,
        } => {
            let v = evaluate_grouped_expr(inner, group_key, group_rows, schema)?;
            crate::type_checker::TypeChecker::coerce_value(&v, target_type)
        }
        Expr::IsNull(inner) => {
            let v = evaluate_grouped_expr(inner, group_key, group_rows, schema)?;
            Ok(LiteralValue::Boolean(matches!(v, LiteralValue::Null)))
        }
        Expr::IsNotNull(inner) => {
            let v = evaluate_grouped_expr(inner, group_key, group_rows, schema)?;
            Ok(LiteralValue::Boolean(!matches!(v, LiteralValue::Null)))
        }
        _ => evaluate_expr(expr, &group_rows[0].values, schema),
    }
}

fn evaluate_aggregate_expr(expr: &Expr, rows: &[Row], schema: &Schema) -> Result<LiteralValue> {
    match expr {
        Expr::Function { name, args } => {
            let lower = name.to_lowercase();
            match lower.as_str() {
                "count" => {
                    if args.len() == 1
                        && matches!(&args[0], Expr::Literal(LiteralValue::String(s)) if s == "*")
                    {
                        return Ok(LiteralValue::Integer(rows.len() as i64));
                    }
                    let mut count = 0i64;
                    for row in rows {
                        for arg in args {
                            let val = evaluate_expr(arg, &row.values, schema)?;
                            if !matches!(val, LiteralValue::Null) {
                                count += 1;
                            }
                        }
                    }
                    Ok(LiteralValue::Integer(count))
                }
                "sum" => {
                    let mut total: f64 = 0.0;
                    let mut has_value = false;
                    for row in rows {
                        for arg in args {
                            let val = evaluate_expr(arg, &row.values, schema)?;
                            match val {
                                LiteralValue::Integer(i) => {
                                    total += i as f64;
                                    has_value = true;
                                }
                                LiteralValue::Float(f) => {
                                    total += f;
                                    has_value = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    if has_value {
                        Ok(LiteralValue::Float(total))
                    } else {
                        Ok(LiteralValue::Null)
                    }
                }
                "avg" => {
                    let mut total: f64 = 0.0;
                    let mut count: usize = 0;
                    for row in rows {
                        for arg in args {
                            let val = evaluate_expr(arg, &row.values, schema)?;
                            match val {
                                LiteralValue::Integer(i) => {
                                    total += i as f64;
                                    count += 1;
                                }
                                LiteralValue::Float(f) => {
                                    total += f;
                                    count += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                    if count > 0 {
                        Ok(LiteralValue::Float(total / count as f64))
                    } else {
                        Ok(LiteralValue::Null)
                    }
                }
                "min" | "max" => {
                    let is_min = lower == "min";
                    let mut best: Option<LiteralValue> = None;
                    for row in rows {
                        for arg in args {
                            let val = evaluate_expr(arg, &row.values, schema)?;
                            if matches!(val, LiteralValue::Null) {
                                continue;
                            }
                            match &best {
                                None => best = Some(val),
                                Some(b) => {
                                    let cmp = if is_min {
                                        crate::execute::eval_binary_op(BinaryOperator::Lt, &val, b)?
                                    } else {
                                        crate::execute::eval_binary_op(BinaryOperator::Gt, &val, b)?
                                    };
                                    if cmp == LiteralValue::Boolean(true) {
                                        best = Some(val);
                                    }
                                }
                            }
                        }
                    }
                    Ok(best.unwrap_or(LiteralValue::Null))
                }
                _ => {
                    let mut last = LiteralValue::Null;
                    for row in rows {
                        let val = evaluate_expr(expr, &row.values, schema)?;
                        last = val;
                    }
                    Ok(last)
                }
            }
        }
        _ => {
            let mut last = LiteralValue::Null;
            for row in rows {
                let val = evaluate_expr(expr, &row.values, schema)?;
                last = val;
            }
            Ok(last)
        }
    }
}

pub struct SortExecutor {
    input: Box<dyn Executor>,
    order_by: Vec<OrderByExpr>,
    schema: Schema,
    rows: Vec<Row>,
    index: usize,
    max_rows: usize,
}

impl SortExecutor {
    pub fn new(input: Box<dyn Executor>, order_by: Vec<OrderByExpr>, schema: Schema) -> Self {
        SortExecutor {
            input,
            order_by,
            schema,
            rows: Vec::new(),
            index: 0,
            max_rows: usize::MAX,
        }
    }

    pub fn with_limit(
        input: Box<dyn Executor>,
        order_by: Vec<OrderByExpr>,
        schema: Schema,
        max_rows: usize,
    ) -> Self {
        SortExecutor {
            input,
            order_by,
            schema,
            rows: Vec::new(),
            index: 0,
            max_rows,
        }
    }
}

impl Executor for SortExecutor {
    fn open(&mut self) -> Result<()> {
        self.input.open()?;
        let mut rows = Vec::new();
        while let Some(row) = self.input.next()? {
            rows.push(row);
            if rows.len() > self.max_rows {
                return Err(crate::error::SQLError::QueryTooComplex(format!(
                    "sort buffer exceeded limit {} (max_batch_size)", 
                    self.max_rows
                )));
            }
        }
        self.input.close()?;
        let schema = self.schema.clone();
        let order_by = self.order_by.clone();
        rows.sort_by(|a, b| {
            for order in &order_by {
                let a_val = sort_key_value(&order.expr, &a.values, &schema);
                let b_val = sort_key_value(&order.expr, &b.values, &schema);
                let cmp = compare_values(&a_val, &b_val, order.nulls_first);
                if cmp != std::cmp::Ordering::Equal {
                    return if order.asc { cmp } else { cmp.reverse() };
                }
            }
            std::cmp::Ordering::Equal
        });
        self.rows = rows;
        self.index = 0;
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Row>> {
        if self.index >= self.rows.len() {
            return Ok(None);
        }
        let row = self.rows[self.index].clone();
        self.index += 1;
        Ok(Some(row))
    }

    fn close(&mut self) -> Result<()> {
        self.rows.clear();
        self.index = 0;
        Ok(())
    }
}

pub struct LimitExecutor {
    input: Box<dyn Executor>,
    limit: usize,
    offset: usize,
    skipped: usize,
    emitted: usize,
}

impl LimitExecutor {
    pub fn new(input: Box<dyn Executor>, limit: usize, offset: usize) -> Self {
        LimitExecutor {
            input,
            limit,
            offset,
            skipped: 0,
            emitted: 0,
        }
    }
}

impl Executor for LimitExecutor {
    fn open(&mut self) -> Result<()> {
        self.input.open()?;
        self.skipped = 0;
        self.emitted = 0;
        while self.skipped < self.offset {
            match self.input.next()? {
                None => break,
                Some(_) => self.skipped += 1,
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Row>> {
        if self.emitted >= self.limit {
            return Ok(None);
        }
        match self.input.next()? {
            None => Ok(None),
            Some(row) => {
                self.emitted += 1;
                Ok(Some(row))
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.input.close()
    }
}

/// Nested-loop INNER JOIN. Buffers the left input, then for each left row
/// iterates the right input. Rows that satisfy the optional ON predicate (or
/// all rows for a CROSS join) are emitted as a concatenated row.
pub struct JoinExecutor {
    left: Box<dyn Executor>,
    right: Box<dyn Executor>,
    on: Option<Expr>,
    schema: Schema,
    left_rows: Vec<Row>,
    left_index: usize,
    right_rows: Vec<Row>,
    right_index: usize,
}

impl JoinExecutor {
    pub fn new(
        left: Box<dyn Executor>,
        right: Box<dyn Executor>,
        on: Option<Expr>,
        schema: Schema,
    ) -> Self {
        JoinExecutor {
            left,
            right,
            on,
            schema,
            left_rows: Vec::new(),
            left_index: 0,
            right_rows: Vec::new(),
            right_index: 0,
        }
    }
}

impl Executor for JoinExecutor {
    fn open(&mut self) -> Result<()> {
        self.left.open()?;
        let mut left_rows = Vec::new();
        while let Some(row) = self.left.next()? {
            left_rows.push(row);
        }
        self.left.close()?;
        self.left_rows = left_rows;

        self.right.open()?;
        let mut right_rows = Vec::new();
        while let Some(row) = self.right.next()? {
            right_rows.push(row);
        }
        self.right.close()?;
        self.right_rows = right_rows;

        self.left_index = 0;
        self.right_index = 0;
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Row>> {
        let schema = self.schema.clone();
        let on = self.on.clone();
        loop {
            if self.left_index >= self.left_rows.len() {
                return Ok(None);
            }
            let left_row = &self.left_rows[self.left_index];
            if self.right_index >= self.right_rows.len() {
                self.left_index += 1;
                self.right_index = 0;
                continue;
            }
            let right_row = &self.right_rows[self.right_index];
            self.right_index += 1;

            let mut values = Vec::with_capacity(left_row.len() + right_row.len());
            values.extend_from_slice(&left_row.values);
            values.extend_from_slice(&right_row.values);
            let combined = Row::new(values);

            match &on {
                None => return Ok(Some(combined)),
                Some(predicate) => {
                    let ok = evaluate_expr(predicate, &combined.values, &schema)?
                        == LiteralValue::Boolean(true);
                    if ok {
                        return Ok(Some(combined));
                    }
                }
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.left_rows.clear();
        self.right_rows.clear();
        self.left_index = 0;
        self.right_index = 0;
        Ok(())
    }
}

/// Resolve the sort key for a single row. `ORDER BY <integer>` is treated as a
/// 1-based ordinal into the projected row; anything else is evaluated as an
/// expression against the output schema (aliases resolve by column name).
fn sort_key_value(expr: &Expr, row: &[Option<LiteralValue>], schema: &Schema) -> LiteralValue {
    if let Expr::Literal(LiteralValue::Integer(ordinal)) = expr
        && *ordinal >= 1
        && (*ordinal as usize) <= row.len()
    {
        return row[(*ordinal as usize) - 1]
            .clone()
            .unwrap_or(LiteralValue::Null);
    }
    match evaluate_expr(expr, row, schema) {
        Ok(v) => v,
        Err(_) => LiteralValue::Null,
    }
}

fn compare_values(
    a: &LiteralValue,
    b: &LiteralValue,
    nulls_first: Option<bool>,
) -> std::cmp::Ordering {
    let nulls_first = nulls_first.unwrap_or(false);
    match (a, b) {
        (LiteralValue::Null, LiteralValue::Null) => std::cmp::Ordering::Equal,
        (LiteralValue::Null, _) => {
            if nulls_first {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }
        (_, LiteralValue::Null) => {
            if nulls_first {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }
        (LiteralValue::Integer(x), LiteralValue::Integer(y)) => x.cmp(y),
        (LiteralValue::Integer(x), LiteralValue::Float(y)) => (*x as f64).total_cmp(y),
        (LiteralValue::Float(x), LiteralValue::Integer(y)) => x.total_cmp(&(*y as f64)),
        (LiteralValue::Float(x), LiteralValue::Float(y)) => x.total_cmp(y),
        (LiteralValue::Boolean(x), LiteralValue::Boolean(y)) => x.cmp(y),
        (LiteralValue::String(x), LiteralValue::String(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}
