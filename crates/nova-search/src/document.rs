use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct IndexedDocument {
    pub id: String,
    pub fields: Vec<IndexedField>,
}

#[derive(Debug, Clone)]
pub struct IndexedField {
    pub name: String,
    pub value: FieldValue,
    pub field_type: FieldType,
    pub boost: f64,
}

#[derive(Debug, Clone)]
pub enum FieldValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Integer,
    Float,
    Bool,
}

impl IndexedDocument {
    pub fn new(id: impl Into<String>) -> Self {
        IndexedDocument {
            id: id.into(),
            fields: Vec::new(),
        }
    }

    pub fn add_text(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push(IndexedField {
            name: name.into(),
            value: FieldValue::Text(value.into()),
            field_type: FieldType::Text,
            boost: 1.0,
        });
        self
    }

    pub fn add_integer(mut self, name: impl Into<String>, value: i64) -> Self {
        self.fields.push(IndexedField {
            name: name.into(),
            value: FieldValue::Integer(value),
            field_type: FieldType::Integer,
            boost: 1.0,
        });
        self
    }

    pub fn add_float(mut self, name: impl Into<String>, value: f64) -> Self {
        self.fields.push(IndexedField {
            name: name.into(),
            value: FieldValue::Float(value),
            field_type: FieldType::Float,
            boost: 1.0,
        });
        self
    }

    pub fn text_value(&self, field_name: &str) -> Option<&str> {
        self.fields.iter().find_map(|f| {
            if f.name == field_name {
                match &f.value {
                    FieldValue::Text(s) => Some(s.as_str()),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    pub fn stored_fields(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for field in &self.fields {
            match &field.value {
                FieldValue::Text(s) => {
                    map.insert(field.name.clone(), s.clone());
                }
                FieldValue::Integer(n) => {
                    map.insert(field.name.clone(), n.to_string());
                }
                FieldValue::Float(f) => {
                    map.insert(field.name.clone(), f.to_string());
                }
                FieldValue::Bool(b) => {
                    map.insert(field.name.clone(), b.to_string());
                }
            }
        }
        map
    }
}
