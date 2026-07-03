use crate::ast::SQLType;

#[derive(Debug, Clone, Default)]
pub struct Schema {
    pub columns: Vec<ColumnInfo>,
}

impl Schema {
    pub fn new(columns: Vec<ColumnInfo>) -> Self {
        Schema { columns }
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn find_column(&self, name: &str) -> Option<&ColumnInfo> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn find_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub sql_type: SQLType,
    pub nullable: bool,
    pub ordinal: usize,
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub schema: Schema,
}
