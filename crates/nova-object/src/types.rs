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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::types::*;
    use crate::schema::FieldDef;

    // --- NovaType variants ---

    #[test]
    fn test_novatype_null() {
        assert_eq!(format!("{:?}", NovaType::Null), "Null");
    }

    #[test]
    fn test_novatype_bool() {
        assert_eq!(format!("{:?}", NovaType::Bool), "Bool");
    }

    #[test]
    fn test_novatype_int_sizes() {
        assert_eq!(format!("{:?}", NovaType::Int8), "Int8");
        assert_eq!(format!("{:?}", NovaType::Int16), "Int16");
        assert_eq!(format!("{:?}", NovaType::Int32), "Int32");
        assert_eq!(format!("{:?}", NovaType::Int64), "Int64");
    }

    #[test]
    fn test_novatype_uint_sizes() {
        assert_eq!(format!("{:?}", NovaType::UInt8), "UInt8");
        assert_eq!(format!("{:?}", NovaType::UInt16), "UInt16");
        assert_eq!(format!("{:?}", NovaType::UInt32), "UInt32");
        assert_eq!(format!("{:?}", NovaType::UInt64), "UInt64");
    }

    #[test]
    fn test_novatype_float_sizes() {
        assert_eq!(format!("{:?}", NovaType::Float32), "Float32");
        assert_eq!(format!("{:?}", NovaType::Float64), "Float64");
    }

    #[test]
    fn test_novatype_string() {
        let t = NovaType::String { max_length: Some(100) };
        assert_eq!(format!("{:?}", t), "String { max_length: Some(100) }");
        let t2 = NovaType::String { max_length: None };
        assert_eq!(format!("{:?}", t2), "String { max_length: None }");
    }

    #[test]
    fn test_novatype_binary() {
        let t = NovaType::Binary { max_length: Some(256) };
        assert_eq!(format!("{:?}", t), "Binary { max_length: Some(256) }");
    }

    #[test]
    fn test_novatype_date_time() {
        assert_eq!(format!("{:?}", NovaType::Date), "Date");
        assert_eq!(format!("{:?}", NovaType::Time), "Time");
        assert_eq!(format!("{:?}", NovaType::DateTime), "DateTime");
        assert_eq!(format!("{:?}", NovaType::Duration), "Duration");
        assert_eq!(format!("{:?}", NovaType::Timestamp), "Timestamp");
    }

    #[test]
    fn test_novatype_decimal() {
        let t = NovaType::Decimal { precision: 10, scale: 2 };
        assert_eq!(format!("{:?}", t), "Decimal { precision: 10, scale: 2 }");
    }

    #[test]
    fn test_novatype_array() {
        let t = NovaType::Array { element_type: Box::new(NovaType::Int32), max_items: Some(10) };
        assert!(format!("{:?}", t).contains("Array"));
    }

    #[test]
    fn test_novatype_object() {
        let fields = vec![
            FieldDef {
                name: "name".into(),
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
        ];
        let t = NovaType::Object { fields, additional_fields: false };
        assert!(format!("{:?}", t).contains("Object"));
    }

    #[test]
    fn test_novatype_map() {
        let t = NovaType::Map { value_type: Box::new(NovaType::String { max_length: None }) };
        assert!(format!("{:?}", t).contains("Map"));
    }

    #[test]
    fn test_novatype_reference() {
        let t = NovaType::Reference { collection: "users".into() };
        assert!(format!("{:?}", t).contains("Reference"));
    }

    #[test]
    fn test_novatype_any() {
        assert_eq!(format!("{:?}", NovaType::Any), "Any");
    }

    #[test]
    fn test_novatype_union() {
        let t = NovaType::Union(vec![NovaType::String { max_length: None }, NovaType::Int64]);
        assert!(format!("{:?}", t).contains("Union"));
    }

    #[test]
    fn test_novatype_optional() {
        let t = NovaType::Optional(Box::new(NovaType::String { max_length: None }));
        assert!(format!("{:?}", t).contains("Optional"));
    }

    #[test]
    fn test_novatype_geo() {
        assert_eq!(format!("{:?}", NovaType::GeoPoint), "GeoPoint");
        assert_eq!(format!("{:?}", NovaType::GeoShape), "GeoShape");
    }

    #[test]
    fn test_novatype_vector() {
        let t = NovaType::Vector { dimensions: 128 };
        assert_eq!(format!("{:?}", t), "Vector { dimensions: 128 }");
    }

    // --- Value type_name ---

    #[test]
    fn test_value_type_name_null() {
        assert_eq!(Value::Null.type_name(), "null");
    }

    #[test]
    fn test_value_type_name_bool() {
        assert_eq!(Value::Bool(true).type_name(), "bool");
    }

    #[test]
    fn test_value_type_name_ints() {
        assert_eq!(Value::Int8(1).type_name(), "int8");
        assert_eq!(Value::Int16(1).type_name(), "int16");
        assert_eq!(Value::Int32(1).type_name(), "int32");
        assert_eq!(Value::Int64(1).type_name(), "int64");
    }

    #[test]
    fn test_value_type_name_uints() {
        assert_eq!(Value::UInt8(1).type_name(), "uint8");
        assert_eq!(Value::UInt16(1).type_name(), "uint16");
        assert_eq!(Value::UInt32(1).type_name(), "uint32");
        assert_eq!(Value::UInt64(1).type_name(), "uint64");
    }

    #[test]
    fn test_value_type_name_floats() {
        assert_eq!(Value::Float32(1.0).type_name(), "float32");
        assert_eq!(Value::Float64(1.0).type_name(), "float64");
    }

    #[test]
    fn test_value_type_name_string() {
        assert_eq!(Value::String("hi".into()).type_name(), "string");
    }

    #[test]
    fn test_value_type_name_binary() {
        assert_eq!(Value::Binary(vec![1, 2, 3]).type_name(), "binary");
    }

    #[test]
    fn test_value_type_name_date() {
        assert_eq!(Value::Date { year: 2024, month: 1, day: 15 }.type_name(), "date");
    }

    #[test]
    fn test_value_type_name_time() {
        assert_eq!(Value::Time { hour: 10, min: 30, sec: 0, nano: 0 }.type_name(), "time");
    }

    #[test]
    fn test_value_type_name_datetime() {
        assert_eq!(Value::DateTime { secs: 0, nsecs: 0 }.type_name(), "datetime");
    }

    #[test]
    fn test_value_type_name_duration() {
        assert_eq!(Value::Duration { nanos: 1000 }.type_name(), "duration");
    }

    #[test]
    fn test_value_type_name_timestamp() {
        assert_eq!(Value::Timestamp(1234567890).type_name(), "timestamp");
    }

    #[test]
    fn test_value_type_name_decimal() {
        assert_eq!(Value::Decimal { value: [0; 16], precision: 10, scale: 2 }.type_name(), "decimal");
    }

    #[test]
    fn test_value_type_name_array() {
        assert_eq!(Value::Array(vec![]).type_name(), "array");
    }

    #[test]
    fn test_value_type_name_object() {
        assert_eq!(Value::Object(HashMap::new()).type_name(), "object");
    }

    #[test]
    fn test_value_type_name_map() {
        assert_eq!(Value::Map(HashMap::new()).type_name(), "map");
    }

    #[test]
    fn test_value_type_name_reference() {
        assert_eq!(Value::Reference { collection: "c".into(), id: [0; 16] }.type_name(), "reference");
    }

    #[test]
    fn test_value_type_name_geopoint() {
        assert_eq!(Value::GeoPoint { lat: 1.0, lon: 2.0 }.type_name(), "geopoint");
    }

    #[test]
    fn test_value_type_name_geoshape() {
        assert_eq!(Value::GeoShape(GeoJsonGeometry::Point { coordinates: [1.0, 2.0] }).type_name(), "geo_shape");
    }

    #[test]
    fn test_value_type_name_vector() {
        assert_eq!(Value::Vector(vec![1.0, 2.0]).type_name(), "vector");
    }

    // --- Value as_i64 ---

    #[test]
    fn test_value_as_i64_int_types() {
        assert_eq!(Value::Int8(-8).as_i64(), Some(-8));
        assert_eq!(Value::Int16(-16).as_i64(), Some(-16));
        assert_eq!(Value::Int32(-32).as_i64(), Some(-32));
        assert_eq!(Value::Int64(-64).as_i64(), Some(-64));
    }

    #[test]
    fn test_value_as_i64_uint_types() {
        assert_eq!(Value::UInt8(8).as_i64(), Some(8));
        assert_eq!(Value::UInt16(16).as_i64(), Some(16));
        assert_eq!(Value::UInt32(32).as_i64(), Some(32));
        assert_eq!(Value::UInt64(64).as_i64(), Some(64));
    }

    #[test]
    fn test_value_as_i64_non_integer_returns_none() {
        assert_eq!(Value::Null.as_i64(), None);
        assert_eq!(Value::Bool(true).as_i64(), None);
        assert_eq!(Value::Float64(1.5).as_i64(), None);
        assert_eq!(Value::String("hi".into()).as_i64(), None);
    }

    // --- Value as_f64 ---

    #[test]
    fn test_value_as_f64_float_types() {
        assert_eq!(Value::Float32(1.5).as_f64(), Some(1.5));
        assert_eq!(Value::Float64(2.5).as_f64(), Some(2.5));
    }

    #[test]
    fn test_value_as_f64_int_types() {
        assert_eq!(Value::Int8(3).as_f64(), Some(3.0));
        assert_eq!(Value::Int64(42).as_f64(), Some(42.0));
        assert_eq!(Value::UInt64(100).as_f64(), Some(100.0));
    }

    #[test]
    fn test_value_as_f64_non_numeric_returns_none() {
        assert_eq!(Value::Null.as_f64(), None);
        assert_eq!(Value::String("x".into()).as_f64(), None);
    }

    // --- Value as_str ---

    #[test]
    fn test_value_as_str_string() {
        assert_eq!(Value::String("hello".into()).as_str(), Some("hello"));
    }

    #[test]
    fn test_value_as_str_non_string_returns_none() {
        assert_eq!(Value::Null.as_str(), None);
        assert_eq!(Value::Bool(true).as_str(), None);
        assert_eq!(Value::Int64(5).as_str(), None);
    }

    // --- Value as_bytes / into_bytes ---

    #[test]
    fn test_value_as_bytes() {
        let data = vec![1, 2, 3];
        assert_eq!(Value::Binary(data.clone()).as_bytes(), Some(&[1u8, 2, 3][..]));
    }

    #[test]
    fn test_value_as_bytes_non_binary_returns_none() {
        assert_eq!(Value::Null.as_bytes(), None);
    }

    #[test]
    fn test_value_into_bytes() {
        let data = vec![10, 20, 30];
        assert_eq!(Value::Binary(data.clone()).into_bytes(), Some(data));
    }

    #[test]
    fn test_value_into_bytes_non_binary_returns_none() {
        assert_eq!(Value::Null.into_bytes(), None);
    }

    // --- Value to_json / from_json round-trip ---

    #[test]
    fn test_json_roundtrip_null() {
        let v = Value::Null;
        let json = v.to_json();
        let back = Value::from_json(json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_roundtrip_bool() {
        let v = Value::Bool(true);
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_roundtrip_int() {
        let v = Value::Int64(42);
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_roundtrip_string() {
        let v = Value::String("hello".into());
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_roundtrip_array() {
        let v = Value::Array(vec![Value::Int64(1), Value::String("two".into())]);
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_roundtrip_object() {
        let mut map = HashMap::new();
        map.insert("key".into(), Value::String("val".into()));
        let v = Value::Object(map);
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_roundtrip_float() {
        let v = Value::Float64(3.14);
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_json_uint64_roundtrip() {
        // u64::MAX loses precision as JSON number (serde_json uses f64 internally)
        let v = Value::UInt64(u64::MAX);
        let json = v.to_json();
        let back = Value::from_json(json).unwrap();
        // u64::MAX becomes Float64 due to precision loss
        assert!(matches!(back, Value::Float64(_)));
    }

    #[test]
    fn test_json_uint64_small_becomes_int64() {
        // UInt64(42) round-trips as Int64(42) because from_json uses as_i64 first
        let v = Value::UInt64(42);
        let back = Value::from_json(v.to_json()).unwrap();
        assert_eq!(back, Value::Int64(42));
    }

    // --- Value validate_type ---

    #[test]
    fn test_validate_type_null() {
        assert!(Value::Null.validate_type(&NovaType::Null));
        assert!(!Value::Null.validate_type(&NovaType::Bool));
    }

    #[test]
    fn test_validate_type_bool() {
        assert!(Value::Bool(true).validate_type(&NovaType::Bool));
        assert!(!Value::Bool(true).validate_type(&NovaType::Int64));
    }

    #[test]
    fn test_validate_type_ints() {
        assert!(Value::Int8(1).validate_type(&NovaType::Int8));
        assert!(Value::Int16(1).validate_type(&NovaType::Int16));
        assert!(Value::Int32(1).validate_type(&NovaType::Int32));
        assert!(Value::Int64(1).validate_type(&NovaType::Int64));
        assert!(!Value::Int64(1).validate_type(&NovaType::String { max_length: None }));
    }

    #[test]
    fn test_validate_type_uints() {
        assert!(Value::UInt8(1).validate_type(&NovaType::UInt8));
        assert!(Value::UInt16(1).validate_type(&NovaType::UInt16));
        assert!(Value::UInt32(1).validate_type(&NovaType::UInt32));
        assert!(Value::UInt64(1).validate_type(&NovaType::UInt64));
    }

    #[test]
    fn test_validate_type_floats() {
        assert!(Value::Float32(1.0).validate_type(&NovaType::Float32));
        assert!(Value::Float64(1.0).validate_type(&NovaType::Float64));
        assert!(!Value::Float64(1.0).validate_type(&NovaType::Int64));
    }

    #[test]
    fn test_validate_type_string() {
        assert!(Value::String("hi".into()).validate_type(&NovaType::String { max_length: None }));
        assert!(!Value::String("hi".into()).validate_type(&NovaType::Bool));
    }

    #[test]
    fn test_validate_type_binary() {
        assert!(Value::Binary(vec![1]).validate_type(&NovaType::Binary { max_length: None }));
    }

    #[test]
    fn test_validate_type_date() {
        assert!(Value::Date { year: 2024, month: 1, day: 1 }.validate_type(&NovaType::Date));
    }

    #[test]
    fn test_validate_type_time() {
        assert!(Value::Time { hour: 12, min: 0, sec: 0, nano: 0 }.validate_type(&NovaType::Time));
    }

    #[test]
    fn test_validate_type_datetime() {
        assert!(Value::DateTime { secs: 0, nsecs: 0 }.validate_type(&NovaType::DateTime));
    }

    #[test]
    fn test_validate_type_duration() {
        assert!(Value::Duration { nanos: 0 }.validate_type(&NovaType::Duration));
    }

    #[test]
    fn test_validate_type_timestamp() {
        assert!(Value::Timestamp(0).validate_type(&NovaType::Timestamp));
    }

    #[test]
    fn test_validate_type_decimal() {
        assert!(Value::Decimal { value: [0; 16], precision: 10, scale: 2 }.validate_type(&NovaType::Decimal { precision: 10, scale: 2 }));
    }

    #[test]
    fn test_validate_type_array() {
        let items = vec![Value::Int64(1), Value::Int64(2)];
        let t = NovaType::Array { element_type: Box::new(NovaType::Int64), max_items: None };
        assert!(Value::Array(items.clone()).validate_type(&t));
    }

    #[test]
    fn test_validate_type_array_max_items() {
        let items = vec![Value::Int64(1), Value::Int64(2), Value::Int64(3)];
        let t = NovaType::Array { element_type: Box::new(NovaType::Int64), max_items: Some(2) };
        assert!(!Value::Array(items).validate_type(&t));
    }

    #[test]
    fn test_validate_type_array_type_mismatch() {
        let items = vec![Value::String("hi".into())];
        let t = NovaType::Array { element_type: Box::new(NovaType::Int64), max_items: None };
        assert!(!Value::Array(items).validate_type(&t));
    }

    #[test]
    fn test_validate_type_object() {
        let mut map = HashMap::new();
        map.insert("name".into(), Value::String("alice".into()));
        let fields = vec![
            FieldDef {
                name: "name".into(),
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
        ];
        let t = NovaType::Object { fields, additional_fields: false };
        assert!(Value::Object(map).validate_type(&t));
    }

    #[test]
    fn test_validate_type_object_missing_required() {
        let map = HashMap::new();
        let fields = vec![
            FieldDef {
                name: "name".into(),
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
        ];
        let t = NovaType::Object { fields, additional_fields: false };
        assert!(!Value::Object(map).validate_type(&t));
    }

    #[test]
    fn test_validate_type_object_unknown_field() {
        let mut map = HashMap::new();
        map.insert("extra".into(), Value::Int64(1));
        let fields = vec![];
        let t = NovaType::Object { fields, additional_fields: false };
        assert!(!Value::Object(map).validate_type(&t));
    }

    #[test]
    fn test_validate_type_map() {
        let mut map = HashMap::new();
        map.insert("k".into(), Value::Int64(1));
        let t = NovaType::Map { value_type: Box::new(NovaType::Int64) };
        assert!(Value::Map(map).validate_type(&t));
    }

    #[test]
    fn test_validate_type_reference() {
        let v = Value::Reference { collection: "users".into(), id: [0; 16] };
        assert!(v.validate_type(&NovaType::Reference { collection: "".into() }));
    }

    #[test]
    fn test_validate_type_geopoint() {
        let v = Value::GeoPoint { lat: 1.0, lon: 2.0 };
        assert!(v.validate_type(&NovaType::GeoPoint));
    }

    #[test]
    fn test_validate_type_geoshape() {
        let v = Value::GeoShape(GeoJsonGeometry::Point { coordinates: [1.0, 2.0] });
        assert!(v.validate_type(&NovaType::GeoShape));
    }

    #[test]
    fn test_validate_type_vector() {
        let v = Value::Vector(vec![1.0, 2.0, 3.0]);
        assert!(v.validate_type(&NovaType::Vector { dimensions: 3 }));
        assert!(!v.validate_type(&NovaType::Vector { dimensions: 2 }));
    }

    #[test]
    fn test_validate_type_any() {
        assert!(Value::Null.validate_type(&NovaType::Any));
        assert!(Value::Bool(true).validate_type(&NovaType::Any));
        assert!(Value::Int64(42).validate_type(&NovaType::Any));
    }

    #[test]
    fn test_validate_type_optional() {
        let opt_str = NovaType::Optional(Box::new(NovaType::String { max_length: None }));
        assert!(Value::Null.validate_type(&opt_str));
        assert!(Value::String("hi".into()).validate_type(&opt_str));
        assert!(!Value::Int64(1).validate_type(&opt_str));
    }

    #[test]
    fn test_validate_type_union() {
        let union = NovaType::Union(vec![NovaType::String { max_length: None }, NovaType::Int64]);
        assert!(Value::String("hi".into()).validate_type(&union));
        assert!(Value::Int64(42).validate_type(&union));
        assert!(!Value::Bool(true).validate_type(&union));
    }

    // --- GeoJsonGeometry variants ---

    #[test]
    fn test_geojson_point() {
        let g = GeoJsonGeometry::Point { coordinates: [1.0, 2.0] };
        assert!(format!("{:?}", g).contains("Point"));
    }

    #[test]
    fn test_geojson_multipoint() {
        let g = GeoJsonGeometry::MultiPoint { coordinates: vec![[1.0, 2.0]] };
        assert!(format!("{:?}", g).contains("MultiPoint"));
    }

    #[test]
    fn test_geojson_linestring() {
        let g = GeoJsonGeometry::LineString { coordinates: vec![[1.0, 2.0], [3.0, 4.0]] };
        assert!(format!("{:?}", g).contains("LineString"));
    }

    #[test]
    fn test_geojson_polygon() {
        let g = GeoJsonGeometry::Polygon { coordinates: vec![vec![[0.0, 0.0], [1.0, 1.0]]] };
        assert!(format!("{:?}", g).contains("Polygon"));
    }

    #[test]
    fn test_geojson_geometry_collection() {
        let g = GeoJsonGeometry::GeometryCollection {
            geometries: vec![GeoJsonGeometry::Point { coordinates: [1.0, 2.0] }],
        };
        assert!(format!("{:?}", g).contains("GeometryCollection"));
    }

    // --- Value equality ---

    #[test]
    fn test_value_equality() {
        assert_eq!(Value::Int64(5), Value::Int64(5));
        assert_ne!(Value::Int64(5), Value::Int64(6));
        assert_ne!(Value::Int64(5), Value::String("5".into()));
    }

    #[test]
    fn test_value_clone() {
        let v = Value::String("hello".into());
        let cloned = v.clone();
        assert_eq!(v, cloned);
    }
}
