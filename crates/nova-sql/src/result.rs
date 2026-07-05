use crate::ast::LiteralValue;

#[derive(Debug, Clone)]
pub struct RecordBatch {
    pub columns: Vec<Column>,
    pub num_rows: usize,
    pub column_names: Vec<String>,
}

impl RecordBatch {
    pub fn new(columns: Vec<Column>, num_rows: usize) -> Self {
        let column_names: Vec<String> = (0..columns.len()).map(|i| format!("col_{}", i)).collect();
        RecordBatch { columns, num_rows, column_names }
    }

    pub fn with_names(columns: Vec<Column>, num_rows: usize, column_names: Vec<String>) -> Self {
        RecordBatch { columns, num_rows, column_names }
    }

    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.num_rows == 0
    }

    pub fn get_column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    pub fn get_row(&self, index: usize) -> Option<Vec<Option<LiteralValue>>> {
        if index >= self.num_rows {
            return None;
        }
        let mut row = Vec::with_capacity(self.columns.len());
        for col in &self.columns {
            row.push(col.get(index));
        }
        Some(row)
    }
}

#[derive(Debug, Clone)]
pub enum Column {
    Integer(Vec<Option<i64>>),
    Float(Vec<Option<f64>>),
    Boolean(Vec<Option<bool>>),
    String(Vec<Option<String>>),
    Null(usize),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::Integer(v) => v.len(),
            Column::Float(v) => v.len(),
            Column::Boolean(v) => v.len(),
            Column::String(v) => v.len(),
            Column::Null(n) => *n,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<LiteralValue> {
        match self {
            Column::Integer(v) => v.get(index).map(|x| x.map(LiteralValue::Integer)).flatten(),
            Column::Float(v) => v.get(index).map(|x| x.map(LiteralValue::Float)).flatten(),
            Column::Boolean(v) => v.get(index).map(|x| x.map(LiteralValue::Boolean)).flatten(),
            Column::String(v) => v.get(index).map(|x| x.clone().map(LiteralValue::String)).flatten(),
            Column::Null(_) => Some(LiteralValue::Null),
        }
    }

    pub fn push(&mut self, value: Option<LiteralValue>) {
        match (self, value) {
            (Column::Integer(v), Some(LiteralValue::Integer(x))) => v.push(Some(x)),
            (Column::Integer(v), None) => v.push(None),
            (Column::Float(v), Some(LiteralValue::Float(x))) => v.push(Some(x)),
            (Column::Float(v), None) => v.push(None),
            (Column::Boolean(v), Some(LiteralValue::Boolean(x))) => v.push(Some(x)),
            (Column::Boolean(v), None) => v.push(None),
            (Column::String(v), Some(LiteralValue::String(x))) => v.push(Some(x)),
            (Column::String(v), None) => v.push(None),
            (Column::Null(n), _) => *n += 1,
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub rows_scanned: u64,
    pub rows_returned: u64,
    pub execution_time_ms: u64,
}

impl ExecutionStats {
    pub fn new(rows_scanned: u64, rows_returned: u64, execution_time_ms: u64) -> Self {
        ExecutionStats {
            rows_scanned,
            rows_returned,
            execution_time_ms,
        }
    }
}
