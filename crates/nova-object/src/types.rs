use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::schema::FieldDef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NovaType {
    Null,
    Bool,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
    String { max_length: Option<u32> },
    Binary { max_length: Option<u32> },
    Date,
    Time,
    DateTime,
    Duration,
    Timestamp,
    Decimal { precision: u8, scale: u8 },
    Array { element_type: Box<NovaType>, max_items: Option<u32> },
    Object { fields: Vec<FieldDef>, additional_fields: bool },
    Map { value_type: Box<NovaType> },
    Reference { collection: String },
    Any,
    Union(Vec<NovaType>),
    Optional(Box<NovaType>),
    GeoPoint,
    GeoShape,
    Vector { dimensions: u16 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeoJsonGeometry {
    Point { coordinates: [f64; 2] },
    MultiPoint { coordinates: Vec<[f64; 2]> },
    LineString { coordinates: Vec<[f64; 2]> },
    MultiLineString { coordinates: Vec<Vec<[f64; 2]>> },
    Polygon { coordinates: Vec<Vec<[f64; 2]>> },
    MultiPolygon { coordinates: Vec<Vec<Vec<[f64; 2]>>> },
    GeometryCollection { geometries: Vec<GeoJsonGeometry> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    String(String),
    Binary(Vec<u8>),
    Date { year: i32, month: u8, day: u8 },
    Time { hour: u8, min: u8, sec: u8, nano: u32 },
    DateTime { secs: i64, nsecs: u32 },
    Duration { nanos: i64 },
    Timestamp(i64),
    Decimal { value: [u8; 16], precision: u8, scale: u8 },
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Map(HashMap<String, Value>),
    Reference { collection: String, id: [u8; 16] },
    GeoPoint { lat: f64, lon: f64 },
    GeoShape(GeoJsonGeometry),
    Vector(Vec<f32>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int8(_) => "int8",
            Value::Int16(_) => "int16",
            Value::Int32(_) => "int32",
            Value::Int64(_) => "int64",
            Value::UInt8(_) => "uint8",
            Value::UInt16(_) => "uint16",
            Value::UInt32(_) => "uint32",
            Value::UInt64(_) => "uint64",
            Value::Float32(_) => "float32",
            Value::Float64(_) => "float64",
            Value::String(_) => "string",
            Value::Binary(_) => "binary",
            Value::Date { .. } => "date",
            Value::Time { .. } => "time",
            Value::DateTime { .. } => "datetime",
            Value::Duration { .. } => "duration",
            Value::Timestamp(_) => "timestamp",
            Value::Decimal { .. } => "decimal",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Map(_) => "map",
            Value::Reference { .. } => "reference",
            Value::GeoPoint { .. } => "geopoint",
            Value::GeoShape(_) => "geo_shape",
            Value::Vector(_) => "vector",
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int8(v) => Some(*v as i64),
            Value::Int16(v) => Some(*v as i64),
            Value::Int32(v) => Some(*v as i64),
            Value::Int64(v) => Some(*v),
            Value::UInt8(v) => Some(*v as i64),
            Value::UInt16(v) => Some(*v as i64),
            Value::UInt32(v) => Some(*v as i64),
            Value::UInt64(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float32(v) => Some(*v as f64),
            Value::Float64(v) => Some(*v),
            Value::Int8(v) => Some(*v as f64),
            Value::Int16(v) => Some(*v as f64),
            Value::Int32(v) => Some(*v as f64),
            Value::Int64(v) => Some(*v as f64),
            Value::UInt8(v) => Some(*v as f64),
            Value::UInt16(v) => Some(*v as f64),
            Value::UInt32(v) => Some(*v as f64),
            Value::UInt64(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Binary(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    pub fn into_bytes(self) -> Option<Vec<u8>> {
        match self {
            Value::Binary(b) => Some(b),
            _ => None,
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            Value::Null => JsonValue::Null,
            Value::Bool(b) => JsonValue::Bool(*b),
            Value::Int8(v) => JsonValue::Number((*v as i64).into()),
            Value::Int16(v) => JsonValue::Number((*v as i64).into()),
            Value::Int32(v) => JsonValue::Number((*v as i64).into()),
            Value::Int64(v) => JsonValue::Number((*v).into()),
            Value::UInt8(v) => JsonValue::Number((*v as u64).into()),
            Value::UInt16(v) => JsonValue::Number((*v as u64).into()),
            Value::UInt32(v) => JsonValue::Number((*v as u64).into()),
            Value::UInt64(v) => JsonValue::Number((*v).into()),
            Value::Float32(v) => {
                let n = serde_json::Number::from_f64(*v as f64).unwrap_or(serde_json::Number::from_f64(0.0).unwrap());
                JsonValue::Number(n)
            }
            Value::Float64(v) => {
                let n = serde_json::Number::from_f64(*v).unwrap_or(serde_json::Number::from_f64(0.0).unwrap());
                JsonValue::Number(n)
            }
            Value::String(s) => JsonValue::String(s.clone()),
            Value::Binary(b) => JsonValue::Array(b.iter().map(|&x| JsonValue::Number(x.into())).collect()),
            Value::Date { year, month, day } => {
                JsonValue::String(format!("{:04}-{:02}-{:02}", year, month, day))
            }
            Value::Time { hour, min, sec, nano } => {
                if *nano > 0 {
                    JsonValue::String(format!("{:02}:{:02}:{:02}.{:09}", hour, min, sec, nano))
                } else {
                    JsonValue::String(format!("{:02}:{:02}:{:02}", hour, min, sec))
                }
            }
            Value::DateTime { secs, nsecs } => {
                JsonValue::Object(serde_json::Map::from_iter([
                    ("secs".to_string(), JsonValue::Number((*secs).into())),
                    ("nsecs".to_string(), JsonValue::Number((*nsecs).into())),
                ]))
            }
            Value::Duration { nanos } => {
                JsonValue::Object(serde_json::Map::from_iter([
                    ("nanos".to_string(), JsonValue::Number((*nanos).into())),
                ]))
            }
            Value::Timestamp(ts) => JsonValue::Number((*ts).into()),
            Value::Decimal { value, precision, scale } => {
                let hex_str = hex::encode(value);
                JsonValue::Object(serde_json::Map::from_iter([
                    ("value".to_string(), JsonValue::String(hex_str)),
                    ("precision".to_string(), JsonValue::Number((*precision).into())),
                    ("scale".to_string(), JsonValue::Number((*scale).into())),
                ]))
            }
            Value::Array(items) => JsonValue::Array(items.iter().map(|v| v.to_json()).collect()),
            Value::Object(map) => {
                JsonValue::Object(map.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
            Value::Map(map) => {
                JsonValue::Object(map.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
            Value::Reference { collection, id } => {
                let id_str = uuid::Uuid::from_bytes(*id).to_string();
                JsonValue::Object(serde_json::Map::from_iter([
                    ("$ref".to_string(), JsonValue::String(format!("{}/{}", collection, id_str))),
                    ("collection".to_string(), JsonValue::String(collection.clone())),
                    ("id".to_string(), JsonValue::String(id_str)),
                ]))
            }
            Value::GeoPoint { lat, lon } => {
                JsonValue::Object(serde_json::Map::from_iter([
                    ("type".to_string(), JsonValue::String("Point".to_string())),
                    ("coordinates".to_string(), JsonValue::Array(vec![
                        JsonValue::Number(serde_json::Number::from_f64(*lon).unwrap_or(serde_json::Number::from_f64(0.0).unwrap())),
                        JsonValue::Number(serde_json::Number::from_f64(*lat).unwrap_or(serde_json::Number::from_f64(0.0).unwrap())),
                    ])),
                ]))
            }
            Value::GeoShape(geom) => serde_json::to_value(geom).unwrap_or(JsonValue::Null),
            Value::Vector(vec) => {
                JsonValue::Array(vec.iter().map(|&x| {
                    JsonValue::Number(serde_json::Number::from_f64(x as f64).unwrap_or(serde_json::Number::from_f64(0.0).unwrap()))
                }).collect())
            }
        }
    }

    pub fn from_json(val: JsonValue) -> Result<Self, String> {
        match val {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(b) => Ok(Value::Bool(b)),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if let Some(u) = n.as_u64() {
                        if u <= i64::MAX as u64 {
                            Ok(Value::Int64(i))
                        } else {
                            Ok(Value::UInt64(u))
                        }
                    } else {
                        Ok(Value::Int64(i))
                    }
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Float64(f))
                } else {
                    Err("Invalid number".to_string())
                }
            }
            JsonValue::String(s) => Ok(Value::String(s)),
            JsonValue::Array(items) => {
                let values: Result<Vec<Value>, String> = items.into_iter().map(Value::from_json).collect();
                Ok(Value::Array(values?))
            }
            JsonValue::Object(map) => {
                if map.contains_key("$timestamp") {
                    let ts = map.get("$timestamp")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| "$timestamp must be an i64".to_string())?;
                    Ok(Value::Timestamp(ts))
                } else if map.contains_key("$ref") {
                    let collection = map.get("collection")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let id_str = map.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let uuid = uuid::Uuid::parse_str(id_str).map_err(|e| e.to_string())?;
                    Ok(Value::Reference { collection, id: *uuid.as_bytes() })
                } else if let Some(geom_type) = map.get("type").and_then(|v| v.as_str()) {
                    match geom_type {
                        "Point" => {
                            if let Some(coords) = map.get("coordinates").and_then(|v| v.as_array()) {
                                if coords.len() >= 2 {
                                    let lon = coords[0].as_f64().unwrap_or(0.0);
                                    let lat = coords[1].as_f64().unwrap_or(0.0);
                                    return Ok(Value::GeoPoint { lat, lon });
                                }
                            }
                            Err("Invalid GeoPoint coordinates".to_string())
                        }
                        "MultiPoint" | "LineString" | "MultiLineString" | "Polygon" | "MultiPolygon" | "GeometryCollection" => {
                            let json_val = JsonValue::Object(map);
                            serde_json::from_value(json_val).map(Value::GeoShape).map_err(|e| e.to_string())
                        }
                        _ => {
                            if map.contains_key("secs") && map.contains_key("nsecs") {
                                let secs = map.get("secs").and_then(|v| v.as_i64()).unwrap_or(0);
                                let nsecs = map.get("nsecs").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                Ok(Value::DateTime { secs, nsecs })
                            } else {
                                let values: Result<HashMap<String, Value>, String> = map
                                    .into_iter()
                                    .map(|(k, v)| Ok((k, Value::from_json(v)?)))
                                    .collect();
                                Ok(Value::Object(values?))
                            }
                        }
                    }
                } else if map.contains_key("secs") && map.contains_key("nsecs") {
                    let secs = map.get("secs").and_then(|v| v.as_i64()).unwrap_or(0);
                    let nsecs = map.get("nsecs").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    Ok(Value::DateTime { secs, nsecs })
                } else {
                    let values: Result<HashMap<String, Value>, String> = map
                        .into_iter()
                        .map(|(k, v)| Ok((k, Value::from_json(v)?)))
                        .collect();
                    Ok(Value::Object(values?))
                }
            }
        }
    }

    pub fn validate_type(&self, expected: &NovaType) -> bool {
        match expected {
            NovaType::Any => true,
            NovaType::Optional(inner) => {
                if matches!(self, Value::Null) {
                    return true;
                }
                self.validate_type(inner)
            }
            NovaType::Union(types) => types.iter().any(|t| self.validate_type(t)),
            NovaType::Null => matches!(self, Value::Null),
            NovaType::Bool => matches!(self, Value::Bool(_)),
            NovaType::Int8 => matches!(self, Value::Int8(_)),
            NovaType::Int16 => matches!(self, Value::Int16(_)),
            NovaType::Int32 => matches!(self, Value::Int32(_)),
            NovaType::Int64 => matches!(self, Value::Int64(_)),
            NovaType::UInt8 => matches!(self, Value::UInt8(_)),
            NovaType::UInt16 => matches!(self, Value::UInt16(_)),
            NovaType::UInt32 => matches!(self, Value::UInt32(_)),
            NovaType::UInt64 => matches!(self, Value::UInt64(_)),
            NovaType::Float32 => matches!(self, Value::Float32(_)),
            NovaType::Float64 => matches!(self, Value::Float64(_)),
            NovaType::String { .. } => matches!(self, Value::String(_)),
            NovaType::Binary { .. } => matches!(self, Value::Binary(_)),
            NovaType::Date => matches!(self, Value::Date { .. }),
            NovaType::Time => matches!(self, Value::Time { .. }),
            NovaType::DateTime => matches!(self, Value::DateTime { .. }),
            NovaType::Duration => matches!(self, Value::Duration { .. }),
            NovaType::Timestamp => matches!(self, Value::Timestamp(_)),
            NovaType::Decimal { .. } => matches!(self, Value::Decimal { .. }),
            NovaType::Array { element_type, max_items } => {
                match self {
                    Value::Array(items) => {
                        if let Some(max) = max_items {
                            if items.len() > *max as usize {
                                return false;
                            }
                        }
                        items.iter().all(|item| item.validate_type(element_type))
                    }
                    _ => false,
                }
            }
            NovaType::Object { fields, additional_fields } => {
                match self {
                    Value::Object(map) => {
                        for field in fields {
                            if field.required {
                                if !map.contains_key(&field.name) {
                                    return false;
                                }
                            }
                            if let Some(val) = map.get(&field.name) {
                                if !val.validate_type(&field.field_type) {
                                    return false;
                                }
                            }
                        }
                        if !additional_fields {
                            for key in map.keys() {
                                if !fields.iter().any(|f| f.name == *key) {
                                    return false;
                                }
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
            NovaType::Map { value_type } => {
                match self {
                    Value::Map(map) => map.values().all(|v| v.validate_type(value_type)),
                    _ => false,
                }
            }
            NovaType::Reference { .. } => matches!(self, Value::Reference { .. }),
            NovaType::GeoPoint => matches!(self, Value::GeoPoint { .. }),
            NovaType::GeoShape => matches!(self, Value::GeoShape(_)),
            NovaType::Vector { dimensions } => {
                match self {
                    Value::Vector(vec) => vec.len() == *dimensions as usize,
                    _ => false,
                }
            }
        }
    }
}


