use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{EventError, EventType, Event, Subsystem};

#[derive(Debug, Clone)]
pub enum PatternSegment {
    Literal(String),
    SingleWildcard,
    MultiWildcard,
}

#[derive(Debug, Clone)]
pub struct TopicPattern {
    pub segments: Vec<PatternSegment>,
    pub canonical: String,
}

impl TopicPattern {
    pub fn new(pattern: &str) -> Result<Self, EventError> {
        if pattern.is_empty() {
            return Err(EventError::InvalidPattern("empty pattern".into()));
        }
        let raw_segments: Vec<&str> = pattern.split('.').collect();
        if raw_segments.iter().any(|s| s.is_empty()) {
            return Err(EventError::InvalidPattern(
                format!("empty segment in pattern: {}", pattern),
            ));
        }
        let mut segments = Vec::with_capacity(raw_segments.len());
        let mut has_multi = false;
        for s in &raw_segments {
            if has_multi {
                return Err(EventError::InvalidPattern(
                    format!("'*' must be last segment: {}", pattern),
                ));
            }
            match *s {
                "+" => segments.push(PatternSegment::SingleWildcard),
                "*" => {
                    segments.push(PatternSegment::MultiWildcard);
                    has_multi = true;
                }
                lit => segments.push(PatternSegment::Literal(lit.to_string())),
            }
        }
        Ok(TopicPattern {
            canonical: pattern.to_string(),
            segments,
        })
    }

    pub fn matches(&self, event_type: &EventType) -> bool {
        Self::segments_match(&self.segments, &event_type.segments)
    }

    fn segments_match(pattern: &[PatternSegment], event: &[String]) -> bool {
        match (pattern.first(), event.first()) {
            (Some(PatternSegment::MultiWildcard), _) => true,
            (Some(PatternSegment::SingleWildcard), Some(_)) => {
                Self::segments_match(&pattern[1..], &event[1..])
            }
            (Some(PatternSegment::Literal(p)), Some(e)) if p == e => {
                Self::segments_match(&pattern[1..], &event[1..])
            }
            (None, None) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterExpr {
    FieldEquals { field: Vec<String>, value: serde_json::Value },
    FieldExists { field: Vec<String> },
    FieldMatches { field: Vec<String>, regex: String },
    FieldIn { field: Vec<String>, values: Vec<serde_json::Value> },
    FieldRange { field: Vec<String>, min: serde_json::Value, max: serde_json::Value },
    Not(Box<FilterExpr>),
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
}

fn get_field<'a>(value: &'a serde_json::Value, field: &[String]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in field {
        match current.get(segment) {
            Some(v) => current = v,
            None => return None,
        }
    }
    Some(current)
}

impl FilterExpr {
    pub fn evaluate(&self, payload: &serde_json::Value) -> bool {
        match self {
            FilterExpr::FieldEquals { field, value } => {
                get_field(payload, field).map_or(false, |actual| actual == value)
            }
            FilterExpr::FieldExists { field } => {
                get_field(payload, field).is_some()
            }
            FilterExpr::FieldMatches { field, regex } => {
                match get_field(payload, field) {
                    Some(serde_json::Value::String(s)) => {
                        regex::Regex::new(regex).map_or(false, |re| re.is_match(s))
                    }
                    _ => false,
                }
            }
            FilterExpr::FieldIn { field, values } => {
                get_field(payload, field).map_or(false, |actual| values.contains(actual))
            }
            FilterExpr::FieldRange { field, min, max } => {
                match get_field(payload, field) {
                    Some(actual) => {
                        let above_min = match (actual, min) {
                            (a, m) if a == m => true,
                            (serde_json::Value::Number(a), serde_json::Value::Number(m)) => {
                                a.as_f64() >= m.as_f64()
                            }
                            (serde_json::Value::String(a), serde_json::Value::String(m)) => a >= m,
                            _ => false,
                        };
                        let below_max = match (actual, max) {
                            (a, m) if a == m => true,
                            (serde_json::Value::Number(a), serde_json::Value::Number(m)) => {
                                a.as_f64() <= m.as_f64()
                            }
                            (serde_json::Value::String(a), serde_json::Value::String(m)) => a <= m,
                            _ => false,
                        };
                        above_min && below_max
                    }
                    None => false,
                }
            }
            FilterExpr::Not(inner) => !inner.evaluate(payload),
            FilterExpr::And(children) => children.iter().all(|child| child.evaluate(payload)),
            FilterExpr::Or(children) => children.iter().any(|child| child.evaluate(payload)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentFilter {
    pub expression: FilterExpr,
}

impl ContentFilter {
    pub fn evaluate(&self, event: &Event) -> bool {
        match serde_json::from_slice::<serde_json::Value>(&event.payload) {
            Ok(value) => self.expression.evaluate(&value),
            Err(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryGuarantee {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberId {
    pub id: String,
    pub subsystem: Subsystem,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: Uuid,
    pub subscriber: SubscriberId,
    pub topic: TopicPattern,
    pub content_filter: Option<ContentFilter>,
    pub delivery_guarantee: DeliveryGuarantee,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub queue_capacity: usize,
    pub created_at: u64,
    pub active: bool,
    pub consumer_group: Option<String>,
    pub sender: crossbeam::channel::Sender<Event>,
}
