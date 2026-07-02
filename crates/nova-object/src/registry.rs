use std::collections::HashMap;
use parking_lot::RwLock;
use tracing::debug;

use nova_core::error::{Result, RuntimeError};
use crate::schema::{
    CollectionSchema, SchemaChange, SchemaChangeOp, NovaType,
};

pub struct SchemaRegistry {
    schemas: RwLock<HashMap<String, CollectionSchema>>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        SchemaRegistry {
            schemas: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, schema: CollectionSchema) -> Result<()> {
        let mut schemas = self.schemas.write();
        let name = schema.collection.clone();
        if schemas.contains_key(&name) {
            return Err(RuntimeError::AlreadyExists(format!(
                "schema for collection '{}' already exists",
                name
            )));
        }
        debug!("registered schema for collection '{}' v{}", name, schema.version);
        schemas.insert(name, schema);
        Ok(())
    }

    pub fn get(&self, collection: &str) -> Result<CollectionSchema> {
        let schemas = self.schemas.read();
        schemas.get(collection).cloned().ok_or_else(|| {
            RuntimeError::NotFound(format!("schema for collection '{}' not found", collection))
        })
    }

    pub fn update(&self, collection: &str, new_schema: CollectionSchema) -> Result<()> {
        let mut schemas = self.schemas.write();
        if !schemas.contains_key(collection) {
            return Err(RuntimeError::NotFound(format!(
                "schema for collection '{}' not found",
                collection
            )));
        }
        debug!("updated schema for collection '{}' to v{}", collection, new_schema.version);
        schemas.insert(collection.to_string(), new_schema);
        Ok(())
    }

    pub fn list(&self) -> Vec<String> {
        let schemas = self.schemas.read();
        let mut names: Vec<String> = schemas.keys().cloned().collect();
        names.sort();
        debug!("listed {} schemas", names.len());
        names
    }

    pub fn delete(&self, collection: &str) -> Result<()> {
        let mut schemas = self.schemas.write();
        if schemas.remove(collection).is_none() {
            return Err(RuntimeError::NotFound(format!(
                "schema for collection '{}' not found",
                collection
            )));
        }
        debug!("deleted schema for collection '{}'", collection);
        Ok(())
    }

    pub fn evolve(
        &self,
        collection: &str,
        changes: Vec<SchemaChangeOp>,
        description: &str,
        author: &str,
    ) -> Result<CollectionSchema> {
        let mut schemas = self.schemas.write();
        let old = schemas.get(collection).ok_or_else(|| {
            RuntimeError::NotFound(format!("schema for collection '{}' not found", collection))
        })?;

        let mut new_schema = old.clone();
        new_schema.version = old.version + 1;
        new_schema.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for op in &changes {
            apply_schema_change(&mut new_schema, op)?;
        }

        check_compatibility_internal(old, &new_schema)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let schema_change = SchemaChange {
            version: new_schema.version,
            timestamp,
            changes,
            description: description.to_string(),
            author: author.to_string(),
        };

        new_schema.changelog.push(schema_change);
        let result = new_schema.clone();
        debug!("evolved schema '{}' from v{} to v{} ({} changes)", collection, old.version, new_schema.version, new_schema.changelog.len());
        schemas.insert(collection.to_string(), new_schema);
        Ok(result)
    }

    pub fn check_compatibility(
        &self,
        old: &CollectionSchema,
        new: &CollectionSchema,
    ) -> Result<()> {
        check_compatibility_internal(old, new)
    }
}

fn apply_schema_change(schema: &mut CollectionSchema, op: &SchemaChangeOp) -> Result<()> {
    match op {
        SchemaChangeOp::AddField { field, .. } => {
            if schema.fields.iter().any(|f| f.name == field.name) {
                return Err(RuntimeError::InvalidArgument(format!(
                    "field '{}' already exists in schema",
                    field.name
                )));
            }
            schema.fields.push(field.clone());
            Ok(())
        }
        SchemaChangeOp::MakeOptional { field, .. } => {
            let f = schema
                .fields
                .iter_mut()
                .find(|f| f.name == *field)
                .ok_or_else(|| {
                    RuntimeError::NotFound(format!("field '{}' not found in schema", field))
                })?;
            f.required = false;
            Ok(())
        }
        SchemaChangeOp::WidenField { field, new_type, .. } => {
            let f = schema
                .fields
                .iter_mut()
                .find(|f| f.name == *field)
                .ok_or_else(|| {
                    RuntimeError::NotFound(format!("field '{}' not found in schema", field))
                })?;
            if !is_type_widening(&f.field_type, new_type) {
                return Err(RuntimeError::InvalidArgument(format!(
                    "cannot narrow type for field '{}'",
                    field
                )));
            }
            f.field_type = new_type.clone();
            Ok(())
        }
        SchemaChangeOp::AddIndex { index, .. } => {
            if schema.indexes.iter().any(|i| i.name == index.name) {
                return Err(RuntimeError::AlreadyExists(format!(
                    "index '{}' already exists",
                    index.name
                )));
            }
            schema.indexes.push(index.clone());
            Ok(())
        }
        SchemaChangeOp::AddDefault { field, default, .. } => {
            if !schema.fields.iter().any(|f| f.name == *field) {
                return Err(RuntimeError::NotFound(format!(
                    "field '{}' not found in schema",
                    field
                )));
            }
            schema.defaults.insert(field.clone(), default.clone());
            Ok(())
        }
        SchemaChangeOp::DeprecateField { .. } => {
            Ok(())
        }
        SchemaChangeOp::AddEnumValue { field, value } => {
            // Enum value addition — requires storage-level validation
            debug!("add enum value '{}' to field '{}' (tracked in changelog)", value, field);
            Ok(())
        }
    }
}

fn check_compatibility_internal(old: &CollectionSchema, new: &CollectionSchema) -> Result<()> {
    for new_field in &new.fields {
        match old.fields.iter().find(|f| f.name == new_field.name) {
            None => {} // new field added — allowed
            Some(old_field) => {
                if old_field.required && !new_field.required {
                    return Err(RuntimeError::InvalidArgument(format!(
                        "field '{}': cannot change from required to optional without using MakeOptional",
                        new_field.name
                    )));
                }
                if old_field.required == new_field.required {
                    // same requiredness, that's fine
                }
                if !is_type_widening(&old_field.field_type, &new_field.field_type) {
                    return Err(RuntimeError::InvalidArgument(format!(
                        "field '{}': type narrowing not allowed (was {:?}, would become {:?})",
                        new_field.name, old_field.field_type, new_field.field_type
                    )));
                }
            }
        }
    }

    for old_field in &old.fields {
        if !new.fields.iter().any(|f| f.name == old_field.name) {
            return Err(RuntimeError::InvalidArgument(format!(
                "field '{}': removal of fields is not allowed",
                old_field.name
            )));
        }
    }

    Ok(())
}

fn is_type_widening(old: &NovaType, new: &NovaType) -> bool {
    if old == new {
        return true;
    }
    match (old, new) {
        (NovaType::Int8, NovaType::Int16)
        | (NovaType::Int8, NovaType::Int32)
        | (NovaType::Int8, NovaType::Int64)
        | (NovaType::Int16, NovaType::Int32)
        | (NovaType::Int16, NovaType::Int64)
        | (NovaType::Int32, NovaType::Int64)
        | (NovaType::UInt8, NovaType::UInt16)
        | (NovaType::UInt8, NovaType::UInt32)
        | (NovaType::UInt8, NovaType::UInt64)
        | (NovaType::UInt16, NovaType::UInt32)
        | (NovaType::UInt16, NovaType::UInt64)
        | (NovaType::UInt32, NovaType::UInt64)
        | (NovaType::Float32, NovaType::Float64) => true,

        (NovaType::String { max_length: a }, NovaType::String { max_length: b }) => {
            match (a, b) {
                (Some(a_len), Some(b_len)) => b_len >= a_len,
                (None, _) => true,
                (Some(_), None) => true,
            }
        }
        (NovaType::Binary { max_length: a }, NovaType::Binary { max_length: b }) => {
            match (a, b) {
                (Some(a_len), Some(b_len)) => b_len >= a_len,
                (None, _) => true,
                (Some(_), None) => true,
            }
        }

        (NovaType::Optional(old_inner), NovaType::Optional(new_inner)) => {
            is_type_widening(old_inner, new_inner)
        }
        (old_inner, NovaType::Optional(new_inner)) => {
            is_type_widening(old_inner, new_inner)
        }

        _ => false,
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
