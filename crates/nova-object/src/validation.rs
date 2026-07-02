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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::document::Document;
    use crate::schema::*;
    use crate::types::Value;
    use crate::validation::{ValidationEngine, ValidationError};

    fn make_schema(mode: SchemaMode, fields: Vec<FieldDef>) -> CollectionSchema {
        CollectionSchema {
            version: 1,
            collection: "test".into(),
            description: "".into(),
            mode,
            fields,
            computed_fields: vec![],
            indexes: vec![],
            defaults: HashMap::new(),
            validation: vec![],
            max_document_size: 0,
            metadata: HashMap::new(),
            changelog: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    fn make_field(name: &str, field_type: NovaType, required: bool) -> FieldDef {
        FieldDef {
            name: name.into(),
            field_type,
            required,
            default: None,
            computed: None,
            description: "".into(),
            index: None,
            unique: false,
            sensitive: false,
            validate: vec![],
        }
    }

    // --- Typed mode: required field validation ---

    #[test]
    fn test_validate_missing_required_field() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("name", NovaType::String { max_length: None }, true),
        ]);
        let doc = Document::new("test");
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ValidationError::MissingRequired(_)));
    }

    #[test]
    fn test_validate_required_field_present() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("name", NovaType::String { max_length: None }, true),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("name".into(), Value::String("alice".into()));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_optional_field_missing() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("name", NovaType::String { max_length: None }, false),
        ]);
        let doc = Document::new("test");
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.is_empty());
    }

    // --- Type mismatch ---

    #[test]
    fn test_validate_type_mismatch() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("count", NovaType::Int32, true),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("count".into(), Value::String("not_a_number".into()));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn test_validate_type_correct() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("count", NovaType::Int32, true),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("count".into(), Value::Int32(42));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.is_empty());
    }

    // --- Unknown fields in Typed mode ---

    #[test]
    fn test_validate_unknown_field_in_typed_mode() {
        let schema = make_schema(SchemaMode::Typed, vec![]);
        let mut doc = Document::new("test");
        doc.data.insert("unknown".into(), Value::String("x".into()));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ValidationError::UnknownField(_)));
    }

    #[test]
    fn test_validate_unknown_field_not_reported_for_known() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("known", NovaType::String { max_length: None }, false),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("known".into(), Value::String("val".into()));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.is_empty());
    }

    // --- Dynamic mode ---

    #[test]
    fn test_validate_dynamic_mode_within_limit() {
        let schema = make_schema(SchemaMode::Dynamic { max_fields: 10 }, vec![]);
        let mut doc = Document::new("test");
        for i in 0..5 {
            doc.data.insert(format!("key_{}", i), Value::Int64(i));
        }
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_dynamic_mode_exceeds_limit() {
        let schema = make_schema(SchemaMode::Dynamic { max_fields: 3 }, vec![]);
        let mut doc = Document::new("test");
        for i in 0..5 {
            doc.data.insert(format!("key_{}", i), Value::Int64(i));
        }
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(!errors.is_empty());
    }

    // --- Mixed mode ---

    #[test]
    fn test_validate_mixed_mode_within_dynamic_limit() {
        let schema = make_schema(SchemaMode::Mixed { max_dynamic_fields: 5 }, vec![
            make_field("fixed1", NovaType::Int64, false),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("fixed1".into(), Value::Int64(1));
        doc.data.insert("dyn1".into(), Value::Int64(2));
        doc.data.insert("dyn2".into(), Value::Int64(3));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_mixed_mode_exceeds_dynamic_limit() {
        let schema = make_schema(SchemaMode::Mixed { max_dynamic_fields: 1 }, vec![
            make_field("fixed1", NovaType::Int64, false),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("fixed1".into(), Value::Int64(1));
        doc.data.insert("dyn1".into(), Value::Int64(2));
        doc.data.insert("dyn2".into(), Value::Int64(3));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(!errors.is_empty());
    }

    // --- Size exceeded ---

    #[test]
    fn test_validate_size_exceeded() {
        let mut schema = make_schema(SchemaMode::Typed, vec![]);
        schema.max_document_size = 1;
        let doc = Document::new("test");
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ValidationError::SizeExceeded(_)));
    }

    #[test]
    fn test_validate_size_ok() {
        let mut schema = make_schema(SchemaMode::Typed, vec![]);
        schema.max_document_size = 1_000_000;
        let doc = Document::new("test");
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        // Size won't be exceeded for a small document
        let size_exceeded = errors.iter().any(|e| matches!(e, ValidationError::SizeExceeded(_)));
        assert!(!size_exceeded);
    }

    // --- apply_defaults ---

    #[test]
    fn test_apply_defaults_field_default() {
        let schema = CollectionSchema {
            version: 1,
            collection: "test".into(),
            description: "".into(),
            mode: SchemaMode::Typed,
            fields: vec![
                FieldDef {
                    name: "status".into(),
                    field_type: NovaType::String { max_length: None },
                    required: false,
                    default: Some(Value::String("active".into())),
                    computed: None,
                    description: "".into(),
                    index: None,
                    unique: false,
                    sensitive: false,
                    validate: vec![],
                },
            ],
            computed_fields: vec![],
            indexes: vec![],
            defaults: HashMap::new(),
            validation: vec![],
            max_document_size: 0,
            metadata: HashMap::new(),
            changelog: vec![],
            created_at: 0,
            updated_at: 0,
        };
        let mut doc = Document::new("test");
        ValidationEngine::apply_defaults(&mut doc, &schema);
        assert_eq!(doc.data.get("status"), Some(&Value::String("active".into())));
    }

    #[test]
    fn test_apply_defaults_does_not_overwrite() {
        let schema = CollectionSchema {
            version: 1,
            collection: "test".into(),
            description: "".into(),
            mode: SchemaMode::Typed,
            fields: vec![
                FieldDef {
                    name: "name".into(),
                    field_type: NovaType::String { max_length: None },
                    required: false,
                    default: Some(Value::String("default".into())),
                    computed: None,
                    description: "".into(),
                    index: None,
                    unique: false,
                    sensitive: false,
                    validate: vec![],
                },
            ],
            computed_fields: vec![],
            indexes: vec![],
            defaults: HashMap::new(),
            validation: vec![],
            max_document_size: 0,
            metadata: HashMap::new(),
            changelog: vec![],
            created_at: 0,
            updated_at: 0,
        };
        let mut doc = Document::new("test");
        doc.data.insert("name".into(), Value::String("existing".into()));
        ValidationEngine::apply_defaults(&mut doc, &schema);
        assert_eq!(doc.data.get("name"), Some(&Value::String("existing".into())));
    }

    #[test]
    fn test_apply_defaults_schema_level() {
        let mut schema = make_schema(SchemaMode::Typed, vec![]);
        schema.defaults.insert("region".into(), Value::String("us-east".into()));
        let mut doc = Document::new("test");
        ValidationEngine::apply_defaults(&mut doc, &schema);
        assert_eq!(doc.data.get("region"), Some(&Value::String("us-east".into())));
    }

    // --- validate_type ---

    #[test]
    fn test_validate_type_delegates() {
        assert!(ValidationEngine::validate_type(&Value::Int64(5), &NovaType::Int64));
        assert!(!ValidationEngine::validate_type(&Value::Int64(5), &NovaType::String { max_length: None }));
    }

    // --- validate_rules: Pattern ---

    #[test]
    fn test_validate_pattern_matches() {
        let doc = doc_with("email", Value::String("a@b.com".into()));
        let rules = vec![ValidationRule::Pattern {
            field: "email".into(),
            regex: r"^.+@.+$".into(),
            error_message: "invalid".into(),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_pattern_no_match() {
        let doc = doc_with("email", Value::String("invalid".into()));
        let rules = vec![ValidationRule::Pattern {
            field: "email".into(),
            regex: r"^.+@.+$".into(),
            error_message: "invalid email".into(),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors, vec!["invalid email"]);
    }

    #[test]
    fn test_validate_pattern_invalid_regex() {
        let doc = doc_with("field", Value::String("val".into()));
        let rules = vec![ValidationRule::Pattern {
            field: "field".into(),
            regex: r"[invalid".into(),
            error_message: "should not happen".into(),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Invalid regex"));
    }

    #[test]
    fn test_validate_pattern_non_string_skipped() {
        let doc = doc_with("count", Value::Int64(42));
        let rules = vec![ValidationRule::Pattern {
            field: "count".into(),
            regex: r"^\d+$".into(),
            error_message: "not a string".into(),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    // --- validate_rules: Range ---

    #[test]
    fn test_validate_range_within_bounds() {
        let doc = doc_with("age", Value::Int32(25));
        let rules = vec![ValidationRule::Range {
            field: "age".into(),
            min: Some(Value::Int32(0)),
            max: Some(Value::Int32(150)),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_range_below_min() {
        let doc = doc_with("age", Value::Int32(-1));
        let rules = vec![ValidationRule::Range {
            field: "age".into(),
            min: Some(Value::Int32(0)),
            max: None,
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("less than minimum"));
    }

    #[test]
    fn test_validate_range_above_max() {
        let doc = doc_with("age", Value::Int32(200));
        let rules = vec![ValidationRule::Range {
            field: "age".into(),
            min: None,
            max: Some(Value::Int32(150)),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("exceeds maximum"));
    }

    #[test]
    fn test_validate_range_missing_field_skipped() {
        let doc = Document::new("test");
        let rules = vec![ValidationRule::Range {
            field: "age".into(),
            min: Some(Value::Int32(0)),
            max: None,
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    // --- validate_rules: Length ---

    #[test]
    fn test_validate_length_within_bounds() {
        let doc = doc_with("name", Value::String("alice".into()));
        let rules = vec![ValidationRule::Length {
            field: "name".into(),
            min: Some(1),
            max: Some(100),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_length_below_min() {
        let doc = doc_with("name", Value::String("".into()));
        let rules = vec![ValidationRule::Length {
            field: "name".into(),
            min: Some(1),
            max: None,
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("less than minimum"));
    }

    #[test]
    fn test_validate_length_above_max() {
        let doc = doc_with("name", Value::String("a".repeat(101)));
        let rules = vec![ValidationRule::Length {
            field: "name".into(),
            min: None,
            max: Some(100),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("exceeds maximum"));
    }

    #[test]
    fn test_validate_length_binary() {
        let doc = doc_with("data", Value::Binary(vec![0u8; 5]));
        let rules = vec![ValidationRule::Length {
            field: "data".into(),
            min: Some(1),
            max: Some(10),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_length_non_string_returns_zero() {
        let doc = doc_with("num", Value::Int64(42));
        let rules = vec![ValidationRule::Length {
            field: "num".into(),
            min: Some(1),
            max: None,
        }];
        // Non-string/binary values get length 0, which is below min
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
    }

    // --- validate_rules: ItemCount ---

    #[test]
    fn test_validate_item_count_within_bounds() {
        let doc = doc_with("tags", Value::Array(vec![
            Value::String("a".into()),
            Value::String("b".into()),
        ]));
        let rules = vec![ValidationRule::ItemCount {
            field: "tags".into(),
            min: Some(1),
            max: Some(10),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_item_count_below_min() {
        let doc = doc_with("tags", Value::Array(vec![]));
        let rules = vec![ValidationRule::ItemCount {
            field: "tags".into(),
            min: Some(1),
            max: None,
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("less than minimum"));
    }

    #[test]
    fn test_validate_item_count_above_max() {
        let doc = doc_with("tags", Value::Array(vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]));
        let rules = vec![ValidationRule::ItemCount {
            field: "tags".into(),
            min: None,
            max: Some(2),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("exceeds maximum"));
    }

    #[test]
    fn test_validate_item_count_non_array_returns_zero() {
        let doc = doc_with("num", Value::Int64(42));
        let rules = vec![ValidationRule::ItemCount {
            field: "num".into(),
            min: Some(1),
            max: None,
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
    }

    // --- validate_rules: Compare ---

    #[test]
    fn test_validate_compare_less_than() {
        let mut doc = Document::new("test");
        doc.data.insert("start".into(), Value::Int32(1));
        doc.data.insert("end".into(), Value::Int32(10));
        let rules = vec![ValidationRule::Compare {
            field_a: "start".into(),
            op: ComparisonOp::LessThan,
            field_b: "end".into(),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_compare_fails() {
        let mut doc = Document::new("test");
        doc.data.insert("start".into(), Value::Int32(10));
        doc.data.insert("end".into(), Value::Int32(1));
        let rules = vec![ValidationRule::Compare {
            field_a: "start".into(),
            op: ComparisonOp::LessThan,
            field_b: "end".into(),
        }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
    }

    // --- validate_rules: Unique (no-op at document level) ---

    #[test]
    fn test_validate_unique_does_not_error() {
        let doc = doc_with("email", Value::String("a@b.com".into()));
        let rules = vec![ValidationRule::Unique { field: "email".into(), scope: None }];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert!(errors.is_empty());
    }

    // --- validate_rules: Custom ---

    #[test]
    fn test_validate_custom_rule_not_implemented() {
        let doc = Document::new("test");
        let rules = vec![ValidationRule::Custom("my_rule".into())];
        let errors = ValidationEngine::validate_rules(&doc, &rules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not implemented"));
    }

    // --- Multiple errors ---

    #[test]
    fn test_validate_multiple_errors() {
        let schema = make_schema(SchemaMode::Typed, vec![
            make_field("name", NovaType::String { max_length: None }, true),
            make_field("age", NovaType::Int32, true),
        ]);
        let mut doc = Document::new("test");
        doc.data.insert("name".into(), Value::Int64(42)); // wrong type
        // age is missing
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert!(errors.len() >= 2);
    }

    // --- Nested object validation ---

    #[test]
    fn test_validate_nested_object() {
        let mut schema = make_schema(SchemaMode::Typed, vec![]);
        schema.validation = vec![
            ValidationRule::Pattern {
                field: "nested.email".into(),
                regex: r"^.+@.+$".into(),
                error_message: "nested email invalid".into(),
            },
        ];
        let mut doc = Document::new("test");
        let mut nested = HashMap::new();
        nested.insert("email".into(), Value::String("bad".into()));
        doc.data.insert("nested".into(), Value::Object(nested));
        let errors = ValidationEngine::validate(&doc, &schema).unwrap();
        assert_eq!(errors.len(), 1);
    }

    // --- Helpers ---
    fn doc_with(field: &str, value: Value) -> Document {
        let mut doc = Document::new("test");
        doc.data.insert(field.into(), value);
        doc
    }
}
