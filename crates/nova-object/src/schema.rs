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
