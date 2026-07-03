use std::sync::Arc;

use dashmap::DashMap;

use crate::ast::LiteralValue;
use crate::error::{Result, SQLError};
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<Option<LiteralValue>>,
}

impl Row {
    pub fn new(values: Vec<Option<LiteralValue>>) -> Self {
        Row { values }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Option<LiteralValue>> {
        self.values.get(index)
    }
}

pub struct TableData {
    pub schema: Schema,
    pub rows: Vec<Row>,
    pub next_row_id: u64,
}

pub struct TableStore {
    tables: DashMap<String, TableData>,
}

impl TableStore {
    pub fn new() -> Self {
        TableStore {
            tables: DashMap::new(),
        }
    }

    pub fn create_table(&self, name: &str, schema: Schema) -> Result<()> {
        if self.tables.contains_key(name) {
            return Err(SQLError::Syntax(format!("table already exists: {}", name)));
        }
        self.tables.insert(
            name.to_string(),
            TableData {
                schema,
                rows: Vec::new(),
                next_row_id: 0,
            },
        );
        Ok(())
    }

    pub fn drop_table(&self, name: &str) -> Result<()> {
        self.tables
            .remove(name)
            .ok_or_else(|| SQLError::TableNotFound(name.to_string()))?;
        Ok(())
    }

    pub fn get_schema(&self, name: &str) -> Result<Schema> {
        self.tables
            .get(name)
            .map(|d| d.schema.clone())
            .ok_or_else(|| SQLError::TableNotFound(name.to_string()))
    }

    pub fn insert_row(&self, name: &str, row: Row) -> Result<()> {
        let mut data = self
            .tables
            .get_mut(name)
            .ok_or_else(|| SQLError::TableNotFound(name.to_string()))?;
        if row.values.len() != data.schema.len() {
            return Err(SQLError::Syntax(format!(
                "expected {} columns, got {}",
                data.schema.len(),
                row.values.len()
            )));
        }
        data.rows.push(row);
        data.next_row_id += 1;
        Ok(())
    }

    pub fn insert_rows(&self, name: &str, rows: Vec<Row>) -> Result<()> {
        let mut data = self
            .tables
            .get_mut(name)
            .ok_or_else(|| SQLError::TableNotFound(name.to_string()))?;
        for row in &rows {
            if row.values.len() != data.schema.len() {
                return Err(SQLError::Syntax(format!(
                    "expected {} columns, got {}",
                    data.schema.len(),
                    row.values.len()
                )));
            }
        }
        for row in rows {
            data.rows.push(row);
            data.next_row_id += 1;
        }
        Ok(())
    }

    pub fn scan_rows(&self, name: &str) -> Result<Vec<Row>> {
        let data = self
            .tables
            .get(name)
            .ok_or_else(|| SQLError::TableNotFound(name.to_string()))?;
        Ok(data.rows.clone())
    }

    pub fn num_rows(&self, name: &str) -> Result<usize> {
        let data = self
            .tables
            .get(name)
            .ok_or_else(|| SQLError::TableNotFound(name.to_string()))?;
        Ok(data.rows.len())
    }

    pub fn table_exists(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    pub fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|e| e.key().clone()).collect()
    }
}

pub type TableStoreRef = Arc<TableStore>;
