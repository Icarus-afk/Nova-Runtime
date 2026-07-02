use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub use crate::types::{NovaType, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSchema {
    pub version: u32,
    pub collection: String,
    pub description: String,
    pub mode: SchemaMode,
    pub fields: Vec<FieldDef>,
    pub computed_fields: Vec<ComputedFieldDef>,
    pub indexes: Vec<IndexDef>,
    pub defaults: HashMap<String, Value>,
    pub validation: Vec<ValidationRule>,
    pub max_document_size: u32,
    pub metadata: HashMap<String, String>,
    pub changelog: Vec<SchemaChange>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchemaMode {
    Dynamic { max_fields: u32 },
    Typed,
    Mixed { max_dynamic_fields: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: NovaType,
    pub required: bool,
    pub default: Option<Value>,
    pub computed: Option<ComputedFieldDef>,
    pub description: String,
    pub index: Option<IndexHint>,
    pub unique: bool,
    pub sensitive: bool,
    pub validate: Vec<ValidationRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IndexHint {
    BTree { order: u8 },
    Hash,
    FullText { language: String, tokenizer: String },
    Geospatial,
    Vector { m: u16, ef_construction: u16, distance: DistanceMetric },
    Composite { fields: Vec<String>, order: Vec<SortOrder> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDef {
    pub name: String,
    pub fields: Vec<IndexField>,
    pub unique: bool,
    pub sparse: bool,
    pub index_type: IndexHint,
    pub options: IndexOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexField {
    pub field: String,
    pub order: SortOrder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexOptions {
    pub language: Option<String>,
    pub vector_dimensions: Option<u16>,
    pub vector_distance: Option<DistanceMetric>,
    pub expire_after_seconds: Option<u64>,
    pub partial_filter: Option<String>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        IndexOptions {
            language: None,
            vector_dimensions: None,
            vector_distance: None,
            expire_after_seconds: None,
            partial_filter: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationRule {
    Pattern { field: String, regex: String, error_message: String },
    Range { field: String, min: Option<Value>, max: Option<Value> },
    Length { field: String, min: Option<u32>, max: Option<u32> },
    ItemCount { field: String, min: Option<u32>, max: Option<u32> },
    Compare { field_a: String, op: ComparisonOp, field_b: String },
    Unique { field: String, scope: Option<String> },
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonOp {
    Equals,
    NotEquals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaChange {
    pub version: u32,
    pub timestamp: u64,
    pub changes: Vec<SchemaChangeOp>,
    pub description: String,
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchemaChangeOp {
    AddField { field: FieldDef, reason: String },
    MakeOptional { field: String, reason: String },
    WidenField { field: String, new_type: NovaType, reason: String },
    AddIndex { index: IndexDef, reason: String },
    AddDefault { field: String, default: Value, reason: String },
    DeprecateField { field: String, deprecation_message: String, removal_version: Option<u32> },
    AddEnumValue { field: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputedExpr {
    pub expression: String,
    pub depends_on: Vec<String>,
    pub output_type: NovaType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputedFieldDef {
    pub name: String,
    pub expr: ComputedExpr,
    pub output_type: NovaType,
}

#[cfg(test)]
mod tests {
    use crate::schema::*;
    use crate::types::NovaType;
    use std::collections::HashMap;

    // --- SchemaMode ---

    #[test]
    fn test_schema_mode_dynamic() {
        let m = SchemaMode::Dynamic { max_fields: 100 };
        assert_eq!(format!("{:?}", m), "Dynamic { max_fields: 100 }");
    }

    #[test]
    fn test_schema_mode_typed() {
        assert_eq!(format!("{:?}", SchemaMode::Typed), "Typed");
    }

    #[test]
    fn test_schema_mode_mixed() {
        let m = SchemaMode::Mixed { max_dynamic_fields: 20 };
        assert_eq!(format!("{:?}", m), "Mixed { max_dynamic_fields: 20 }");
    }

    // --- FieldDef ---

    #[test]
    fn test_field_def_creation() {
        let field = FieldDef {
            name: "age".into(),
            field_type: NovaType::Int32,
            required: true,
            default: Some(crate::types::Value::Int32(0)),
            computed: None,
            description: "The age of the user".into(),
            index: None,
            unique: false,
            sensitive: false,
            validate: vec![],
        };
        assert_eq!(field.name, "age");
        assert_eq!(field.field_type, NovaType::Int32);
        assert!(field.required);
        assert_eq!(field.default, Some(crate::types::Value::Int32(0)));
    }

    #[test]
    fn test_field_def_required_vs_optional() {
        let required = FieldDef {
            name: "email".into(),
            field_type: NovaType::String { max_length: None },
            required: true,
            default: None,
            computed: None,
            description: "".into(),
            index: None,
            unique: false,
            sensitive: false,
            validate: vec![],
        };
        let optional = FieldDef {
            required: false,
            ..required.clone()
        };
        assert!(required.required);
        assert!(!optional.required);
    }

    #[test]
    fn test_field_def_default() {
        let field = FieldDef {
            name: "count".into(),
            field_type: NovaType::Int64,
            required: false,
            default: Some(crate::types::Value::Int64(0)),
            computed: None,
            description: "".into(),
            index: None,
            unique: false,
            sensitive: false,
            validate: vec![],
        };
        assert_eq!(field.default, Some(crate::types::Value::Int64(0)));
    }

    #[test]
    fn test_field_def_unique_sensitive() {
        let field = FieldDef {
            name: "ssn".into(),
            field_type: NovaType::String { max_length: None },
            required: true,
            default: None,
            computed: None,
            description: "".into(),
            index: None,
            unique: true,
            sensitive: true,
            validate: vec![],
        };
        assert!(field.unique);
        assert!(field.sensitive);
    }

    // --- IndexHint ---

    #[test]
    fn test_index_hint_btree() {
        let h = IndexHint::BTree { order: 1 };
        assert_eq!(format!("{:?}", h), "BTree { order: 1 }");
    }

    #[test]
    fn test_index_hint_hash() {
        assert_eq!(format!("{:?}", IndexHint::Hash), "Hash");
    }

    #[test]
    fn test_index_hint_fulltext() {
        let h = IndexHint::FullText { language: "en".into(), tokenizer: "standard".into() };
        assert!(format!("{:?}", h).contains("FullText"));
    }

    #[test]
    fn test_index_hint_geospatial() {
        assert_eq!(format!("{:?}", IndexHint::Geospatial), "Geospatial");
    }

    #[test]
    fn test_index_hint_vector() {
        let h = IndexHint::Vector { m: 16, ef_construction: 200, distance: DistanceMetric::Cosine };
        assert!(format!("{:?}", h).contains("Vector"));
    }

    #[test]
    fn test_index_hint_composite() {
        let h = IndexHint::Composite {
            fields: vec!["a".into(), "b".into()],
            order: vec![SortOrder::Ascending, SortOrder::Descending],
        };
        assert!(format!("{:?}", h).contains("Composite"));
    }

    // --- SortOrder ---

    #[test]
    fn test_sort_order_variants() {
        assert_eq!(format!("{:?}", SortOrder::Ascending), "Ascending");
        assert_eq!(format!("{:?}", SortOrder::Descending), "Descending");
    }

    // --- DistanceMetric ---

    #[test]
    fn test_distance_metric_variants() {
        assert_eq!(format!("{:?}", DistanceMetric::Cosine), "Cosine");
        assert_eq!(format!("{:?}", DistanceMetric::Euclidean), "Euclidean");
        assert_eq!(format!("{:?}", DistanceMetric::DotProduct), "DotProduct");
    }

    // --- IndexDef ---

    #[test]
    fn test_index_def_creation() {
        let idx = IndexDef {
            name: "idx_name".into(),
            fields: vec![IndexField { field: "name".into(), order: SortOrder::Ascending }],
            unique: true,
            sparse: false,
            index_type: IndexHint::BTree { order: 1 },
            options: IndexOptions::default(),
        };
        assert_eq!(idx.name, "idx_name");
        assert!(idx.unique);
    }

    // --- IndexOptions ---

    #[test]
    fn test_index_options_default() {
        let opts = IndexOptions::default();
        assert_eq!(opts.language, None);
        assert_eq!(opts.vector_dimensions, None);
        assert_eq!(opts.vector_distance, None);
        assert_eq!(opts.expire_after_seconds, None);
        assert_eq!(opts.partial_filter, None);
    }

    // --- ValidationRule ---

    #[test]
    fn test_validation_rule_pattern() {
        let r = ValidationRule::Pattern {
            field: "email".into(),
            regex: r"^.+@.+$".into(),
            error_message: "invalid email".into(),
        };
        assert!(format!("{:?}", r).contains("Pattern"));
    }

    #[test]
    fn test_validation_rule_range() {
        let r = ValidationRule::Range {
            field: "age".into(),
            min: Some(crate::types::Value::Int32(0)),
            max: Some(crate::types::Value::Int32(150)),
        };
        assert!(format!("{:?}", r).contains("Range"));
    }

    #[test]
    fn test_validation_rule_length() {
        let r = ValidationRule::Length {
            field: "name".into(),
            min: Some(1),
            max: Some(100),
        };
        assert!(format!("{:?}", r).contains("Length"));
    }

    #[test]
    fn test_validation_rule_compare() {
        let r = ValidationRule::Compare {
            field_a: "start".into(),
            op: ComparisonOp::LessThan,
            field_b: "end".into(),
        };
        assert!(format!("{:?}", r).contains("Compare"));
    }

    #[test]
    fn test_validation_rule_unique() {
        let r = ValidationRule::Unique { field: "email".into(), scope: Some("global".into()) };
        assert!(format!("{:?}", r).contains("Unique"));
    }

    #[test]
    fn test_validation_rule_custom() {
        let r = ValidationRule::Custom("my_rule".into());
        assert_eq!(format!("{:?}", r), "Custom(\"my_rule\")");
    }

    // --- ComparisonOp ---

    #[test]
    fn test_comparison_op_variants() {
        assert_eq!(format!("{:?}", ComparisonOp::Equals), "Equals");
        assert_eq!(format!("{:?}", ComparisonOp::NotEquals), "NotEquals");
        assert_eq!(format!("{:?}", ComparisonOp::LessThan), "LessThan");
        assert_eq!(format!("{:?}", ComparisonOp::LessThanOrEqual), "LessThanOrEqual");
        assert_eq!(format!("{:?}", ComparisonOp::GreaterThan), "GreaterThan");
        assert_eq!(format!("{:?}", ComparisonOp::GreaterThanOrEqual), "GreaterThanOrEqual");
    }

    // --- SchemaChange / SchemaChangeOp ---

    #[test]
    fn test_schema_change_op_add_field() {
        let op = SchemaChangeOp::AddField {
            field: FieldDef {
                name: "new_field".into(),
                field_type: NovaType::String { max_length: None },
                required: false,
                default: None,
                computed: None,
                description: "".into(),
                index: None,
                unique: false,
                sensitive: false,
                validate: vec![],
            },
            reason: "needed".into(),
        };
        assert!(format!("{:?}", op).contains("AddField"));
    }

    #[test]
    fn test_schema_change_op_make_optional() {
        let op = SchemaChangeOp::MakeOptional { field: "old_field".into(), reason: "relaxing".into() };
        assert!(format!("{:?}", op).contains("MakeOptional"));
    }

    #[test]
    fn test_schema_change_op_widen_field() {
        let op = SchemaChangeOp::WidenField {
            field: "count".into(),
            new_type: NovaType::Int64,
            reason: "need larger range".into(),
        };
        assert!(format!("{:?}", op).contains("WidenField"));
    }

    #[test]
    fn test_schema_change_op_add_index() {
        let op = SchemaChangeOp::AddIndex {
            index: IndexDef {
                name: "idx".into(),
                fields: vec![],
                unique: false,
                sparse: false,
                index_type: IndexHint::Hash,
                options: IndexOptions::default(),
            },
            reason: "performance".into(),
        };
        assert!(format!("{:?}", op).contains("AddIndex"));
    }

    #[test]
    fn test_schema_change_op_add_default() {
        let op = SchemaChangeOp::AddDefault {
            field: "status".into(),
            default: crate::types::Value::String("active".into()),
            reason: "default value".into(),
        };
        assert!(format!("{:?}", op).contains("AddDefault"));
    }

    #[test]
    fn test_schema_change_op_deprecate_field() {
        let op = SchemaChangeOp::DeprecateField {
            field: "old".into(),
            deprecation_message: "use new instead".into(),
            removal_version: Some(3),
        };
        assert!(format!("{:?}", op).contains("DeprecateField"));
    }

    #[test]
    fn test_schema_change_op_add_enum_value() {
        let op = SchemaChangeOp::AddEnumValue { field: "color".into(), value: "blue".into() };
        assert!(format!("{:?}", op).contains("AddEnumValue"));
    }

    // --- CollectionSchema ---

    #[test]
    fn test_collection_schema_creation() {
        let schema = CollectionSchema {
            version: 1,
            collection: "users".into(),
            description: "User profiles".into(),
            mode: SchemaMode::Typed,
            fields: vec![],
            computed_fields: vec![],
            indexes: vec![],
            defaults: HashMap::new(),
            validation: vec![],
            max_document_size: 16 * 1024 * 1024,
            metadata: HashMap::new(),
            changelog: vec![],
            created_at: 1000,
            updated_at: 1000,
        };
        assert_eq!(schema.version, 1);
        assert_eq!(schema.collection, "users");
        assert_eq!(schema.max_document_size, 16 * 1024 * 1024);
    }

    #[test]
    fn test_collection_schema_with_fields() {
        let schema = CollectionSchema {
            version: 1,
            collection: "products".into(),
            description: "".into(),
            mode: SchemaMode::Typed,
            fields: vec![
                FieldDef {
                    name: "sku".into(),
                    field_type: NovaType::String { max_length: Some(50) },
                    required: true,
                    default: None,
                    computed: None,
                    description: "Stock keeping unit".into(),
                    index: Some(IndexHint::Hash),
                    unique: true,
                    sensitive: false,
                    validate: vec![],
                },
            ],
            computed_fields: vec![],
            indexes: vec![],
            defaults: HashMap::new(),
            validation: vec![],
            max_document_size: 1024,
            metadata: HashMap::new(),
            changelog: vec![],
            created_at: 2000,
            updated_at: 2000,
        };
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(schema.fields[0].name, "sku");
        assert!(schema.fields[0].unique);
    }

    #[test]
    fn test_collection_schema_changelog() {
        let change = SchemaChange {
            version: 2,
            timestamp: 3000,
            changes: vec![SchemaChangeOp::AddField {
                field: FieldDef {
                    name: "email".into(),
                    field_type: NovaType::String { max_length: None },
                    required: true,
                    default: None,
                    computed: None,
                    description: "".into(),
                    index: None,
                    unique: false,
                    sensitive: false,
                    validate: vec![],
                },
                reason: "add email field".into(),
            }],
            description: "Add email field".into(),
            author: "dev".into(),
        };
        assert_eq!(change.version, 2);
        assert_eq!(change.changes.len(), 1);
    }

    // --- ComputedExpr / ComputedFieldDef ---

    #[test]
    fn test_computed_expr() {
        let expr = ComputedExpr {
            expression: "CONCAT(first_name, ' ', last_name)".into(),
            depends_on: vec!["first_name".into(), "last_name".into()],
            output_type: NovaType::String { max_length: None },
        };
        assert_eq!(expr.depends_on.len(), 2);
    }

    #[test]
    fn test_computed_field_def() {
        let field = ComputedFieldDef {
            name: "full_name".into(),
            expr: ComputedExpr {
                expression: "CONCAT(first, ' ', last)".into(),
                depends_on: vec!["first".into(), "last".into()],
                output_type: NovaType::String { max_length: None },
            },
            output_type: NovaType::String { max_length: None },
        };
        assert_eq!(field.name, "full_name");
    }

    // --- Equality / Clone ---

    #[test]
    fn test_field_def_equality() {
        let a = FieldDef {
            name: "x".into(),
            field_type: NovaType::Int32,
            required: true,
            default: None,
            computed: None,
            description: "".into(),
            index: None,
            unique: false,
            sensitive: false,
            validate: vec![],
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_index_def_equality() {
        let a = IndexDef {
            name: "idx".into(),
            fields: vec![],
            unique: false,
            sparse: false,
            index_type: IndexHint::Hash,
            options: IndexOptions::default(),
        };
        assert_eq!(a, a.clone());
    }
}
