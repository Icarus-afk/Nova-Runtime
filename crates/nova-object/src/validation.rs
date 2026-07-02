use regex::Regex;
use thiserror::Error;
use tracing::debug;

use crate::document::Document;
use crate::schema::{CollectionSchema, NovaType, SchemaMode, ValidationRule, ComparisonOp};
use crate::types::Value;

#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    #[error("Missing required field: {0}")]
    MissingRequired(String),
    #[error("Type mismatch for field '{field}': expected {expected:?}, got {actual:?}")]
    TypeMismatch {
        field: String,
        expected: NovaType,
        actual: NovaType,
    },
    #[error("Field '{0}' exceeds maximum document size")]
    SizeExceeded(String),
    #[error("Validation rule failed: {0}")]
    RuleFailed(String),
    #[error("Unknown field '{0}' in Typed schema")]
    UnknownField(String),
}

pub struct ValidationEngine;

impl ValidationEngine {
    pub fn validate(doc: &Document, schema: &CollectionSchema) -> Result<Vec<ValidationError>, String> {
        let mut errors = Vec::new();

        match &schema.mode {
            SchemaMode::Typed => {
                for field in &schema.fields {
                    if field.required {
                        if !doc.data.contains_key(&field.name) {
                            errors.push(ValidationError::MissingRequired(field.name.clone()));
                        }
                    }
                }
                for (key, _) in &doc.data {
                    if !schema.fields.iter().any(|f| f.name == *key)
                        && !schema.computed_fields.iter().any(|f| f.name == *key)
                    {
                        errors.push(ValidationError::UnknownField(key.clone()));
                    }
                }
            }
            SchemaMode::Dynamic { max_fields } => {
                if doc.data.len() > *max_fields as usize {
                    errors.push(ValidationError::RuleFailed(format!(
                        "document has {} fields, max allowed is {}",
                        doc.data.len(),
                        max_fields
                    )));
                }
            }
            SchemaMode::Mixed { max_dynamic_fields } => {
                let typed_field_count = schema.fields.len();
                let dynamic_count = doc.data.len().saturating_sub(typed_field_count);
                if dynamic_count > *max_dynamic_fields as usize {
                    errors.push(ValidationError::RuleFailed(format!(
                        "document has {} dynamic fields, max allowed is {}",
                        dynamic_count, max_dynamic_fields
                    )));
                }
            }
        }

        for field in &schema.fields {
            if let Some(value) = doc.data.get(&field.name) {
                if !value.validate_type(&field.field_type) {
                    errors.push(ValidationError::TypeMismatch {
                        field: field.name.clone(),
                        expected: field.field_type.clone(),
                        actual: value_type(value),
                    });
                }
            } else if field.required {
                errors.push(ValidationError::MissingRequired(field.name.clone()));
            }
        }

        let size = doc.compute_size();
        if schema.max_document_size > 0 && size > schema.max_document_size {
            errors.push(ValidationError::SizeExceeded(doc.meta.collection.clone()));
        }

        let rule_errors = Self::validate_rules(doc, &schema.validation);
        for err in rule_errors {
            errors.push(ValidationError::RuleFailed(err));
        }

        debug!("validated document '{}': {} errors", doc.meta.collection, errors.len());
        Ok(errors)
    }

    pub fn validate_type(value: &Value, expected: &NovaType) -> bool {
        value.validate_type(expected)
    }

    pub fn apply_defaults(doc: &mut Document, schema: &CollectionSchema) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for field in &schema.fields {
            if !doc.data.contains_key(&field.name) {
                if let Some(ref default) = field.default {
                    doc.data.insert(field.name.clone(), default.clone());
                }
            }
        }

        for (field_name, default_value) in &schema.defaults {
            if !doc.data.contains_key(field_name) {
                doc.data.insert(field_name.clone(), default_value.clone());
            }
        }

        doc.meta.updated_at = now;
        debug!("applied defaults to document '{}'", doc.meta.collection);
    }

    pub fn validate_rules(doc: &Document, rules: &[ValidationRule]) -> Vec<String> {
        let mut errors = Vec::new();

        for rule in rules {
            match rule {
                ValidationRule::Pattern { field, regex, error_message } => {
                    if let Some(Value::String(val)) = doc.data.get(field) {
                        match Regex::new(regex) {
                            Ok(re) => {
                                if !re.is_match(val) {
                                    errors.push(error_message.clone());
                                }
                            }
                            Err(e) => {
                                errors.push(format!("Invalid regex '{}': {}", regex, e));
                            }
                        }
                    }
                }
                ValidationRule::Range { field, min, max } => {
                    if let Some(value) = doc.data.get(field) {
                        if let Some(min_val) = min {
                            if !value_compare(value, min_val, ComparisonOp::GreaterThanOrEqual) {
                                errors.push(format!(
                                    "Field '{}' value {:?} is less than minimum {:?}",
                                    field, value, min_val
                                ));
                            }
                        }
                        if let Some(max_val) = max {
                            if !value_compare(value, max_val, ComparisonOp::LessThanOrEqual) {
                                errors.push(format!(
                                    "Field '{}' value {:?} exceeds maximum {:?}",
                                    field, value, max_val
                                ));
                            }
                        }
                    }
                }
                ValidationRule::Length { field, min, max } => {
                    if let Some(value) = doc.data.get(field) {
                        let len = match value {
                            Value::String(s) => s.len() as u32,
                            Value::Binary(b) => b.len() as u32,
                            _ => 0,
                        };
                        if let Some(min_len) = min {
                            if len < *min_len {
                                errors.push(format!(
                                    "Field '{}' length {} is less than minimum {}",
                                    field, len, min_len
                                ));
                            }
                        }
                        if let Some(max_len) = max {
                            if len > *max_len {
                                errors.push(format!(
                                    "Field '{}' length {} exceeds maximum {}",
                                    field, len, max_len
                                ));
                            }
                        }
                    }
                }
                ValidationRule::ItemCount { field, min, max } => {
                    if let Some(value) = doc.data.get(field) {
                        let count = match value {
                            Value::Array(items) => items.len() as u32,
                            _ => 0,
                        };
                        if let Some(min_count) = min {
                            if count < *min_count {
                                errors.push(format!(
                                    "Field '{}' item count {} is less than minimum {}",
                                    field, count, min_count
                                ));
                            }
                        }
                        if let Some(max_count) = max {
                            if count > *max_count {
                                errors.push(format!(
                                    "Field '{}' item count {} exceeds maximum {}",
                                    field, count, max_count
                                ));
                            }
                        }
                    }
                }
                ValidationRule::Compare { field_a, op, field_b } => {
                    if let (Some(val_a), Some(val_b)) = (doc.data.get(field_a), doc.data.get(field_b)) {
                        if !value_compare(val_a, val_b, op.clone()) {
                            errors.push(format!(
                                "Compare rule failed: {:?} {:?} {:?}",
                                val_a, op, val_b
                            ));
                        }
                    }
                }
                ValidationRule::Unique { field, scope: _ } => {
                    if let Some(_) = doc.data.get(field) {
                        // Uniqueness validation requires storage-level checks
                        // At the document level, we only record that a unique constraint exists
                    }
                }
                ValidationRule::Custom(name) => {
                    errors.push(format!("Custom validation rule '{}' not implemented", name));
                }
            }
        }

        errors
    }
}

fn value_type(value: &Value) -> NovaType {
    match value {
        Value::Null => NovaType::Null,
        Value::Bool(_) => NovaType::Bool,
        Value::Int8(_) => NovaType::Int8,
        Value::Int16(_) => NovaType::Int16,
        Value::Int32(_) => NovaType::Int32,
        Value::Int64(_) => NovaType::Int64,
        Value::UInt8(_) => NovaType::UInt8,
        Value::UInt16(_) => NovaType::UInt16,
        Value::UInt32(_) => NovaType::UInt32,
        Value::UInt64(_) => NovaType::UInt64,
        Value::Float32(_) => NovaType::Float32,
        Value::Float64(_) => NovaType::Float64,
        Value::String(_) => NovaType::String { max_length: None },
        Value::Binary(_) => NovaType::Binary { max_length: None },
        Value::Date { .. } => NovaType::Date,
        Value::Time { .. } => NovaType::Time,
        Value::DateTime { .. } => NovaType::DateTime,
        Value::Duration { .. } => NovaType::Duration,
        Value::Timestamp(_) => NovaType::Timestamp,
        Value::Decimal { precision, scale, .. } => NovaType::Decimal { precision: *precision, scale: *scale },
        Value::Array(_) => NovaType::Array {
            element_type: Box::new(NovaType::Any),
            max_items: None,
        },
        Value::Object(_) => NovaType::Object {
            fields: Vec::new(),
            additional_fields: true,
        },
        Value::Map(_) => NovaType::Map {
            value_type: Box::new(NovaType::Any),
        },
        Value::Reference { .. } => NovaType::Reference {
            collection: String::new(),
        },
        Value::GeoPoint { .. } => NovaType::GeoPoint,
        Value::GeoShape(_) => NovaType::GeoShape,
        Value::Vector(vec) => NovaType::Vector {
            dimensions: vec.len() as u16,
        },
    }
}

fn value_compare(a: &Value, b: &Value, op: ComparisonOp) -> bool {
    let ord = match (a, b) {
        (Value::Int8(x), Value::Int8(y)) => Some(x.cmp(y)),
        (Value::Int16(x), Value::Int16(y)) => Some(x.cmp(y)),
        (Value::Int32(x), Value::Int32(y)) => Some(x.cmp(y)),
        (Value::Int64(x), Value::Int64(y)) => Some(x.cmp(y)),
        (Value::UInt8(x), Value::UInt8(y)) => Some(x.cmp(y)),
        (Value::UInt16(x), Value::UInt16(y)) => Some(x.cmp(y)),
        (Value::UInt32(x), Value::UInt32(y)) => Some(x.cmp(y)),
        (Value::UInt64(x), Value::UInt64(y)) => Some(x.cmp(y)),
        (Value::Float32(x), Value::Float32(y)) => x.partial_cmp(y),
        (Value::Float64(x), Value::Float64(y)) => x.partial_cmp(y),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Int64(x), Value::Float64(y)) => (*x as f64).partial_cmp(y),
        (Value::Float64(x), Value::Int64(y)) => x.partial_cmp(&(*y as f64)),
        _ => None,
    };

    match ord {
        Some(o) => match op {
            ComparisonOp::Equals => o == std::cmp::Ordering::Equal,
            ComparisonOp::NotEquals => o != std::cmp::Ordering::Equal,
            ComparisonOp::LessThan => o == std::cmp::Ordering::Less,
            ComparisonOp::LessThanOrEqual => o != std::cmp::Ordering::Greater,
            ComparisonOp::GreaterThan => o == std::cmp::Ordering::Greater,
            ComparisonOp::GreaterThanOrEqual => o != std::cmp::Ordering::Less,
        },
        None => match op {
            ComparisonOp::Equals | ComparisonOp::NotEquals => {
                let eq = std::mem::discriminant(a) == std::mem::discriminant(b);
                match op {
                    ComparisonOp::Equals => eq,
                    ComparisonOp::NotEquals => !eq,
                    _ => false,
                }
            }
            _ => false,
        },
    }
}
