use crate::ast::SQLType;

/// Maps SQLType to a string representation for storage metadata.
pub fn sql_type_name(t: &SQLType) -> &'static str {
    match t {
        SQLType::Null => "NULL",
        SQLType::Boolean => "BOOLEAN",
        SQLType::Integer => "INTEGER",
        SQLType::Float => "FLOAT",
        SQLType::Text => "TEXT",
    }
}

/// Maps a string type name to SQLType.
pub fn name_to_sql_type(name: &str) -> Option<SQLType> {
    match name.to_lowercase().as_str() {
        "null" => Some(SQLType::Null),
        "boolean" | "bool" => Some(SQLType::Boolean),
        "integer" | "int" => Some(SQLType::Integer),
        "float" | "double" | "real" => Some(SQLType::Float),
        "text" | "varchar" | "string" => Some(SQLType::Text),
        _ => None,
    }
}
