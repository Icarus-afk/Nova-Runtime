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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventBuilder;

    #[test]
    fn test_topic_pattern_literal_matches() {
        let pattern = TopicPattern::new("test.event.created").unwrap();
        let event_type = EventType::new("test.event.created").unwrap();
        assert!(pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_literal_no_match() {
        let pattern = TopicPattern::new("test.event.created").unwrap();
        let event_type = EventType::new("test.event.deleted").unwrap();
        assert!(!pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_single_wildcard_matches() {
        let pattern = TopicPattern::new("test.+.created").unwrap();
        let event_type = EventType::new("test.event.created").unwrap();
        assert!(pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_single_wildcard_no_match() {
        let pattern = TopicPattern::new("test.+.created").unwrap();
        let event_type = EventType::new("test.event.extra.created").unwrap();
        assert!(!pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_multi_wildcard_matches() {
        let pattern = TopicPattern::new("test.*").unwrap();
        let event_type = EventType::new("test.event.created").unwrap();
        assert!(pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_multi_wildcard_matches_any_depth() {
        let pattern = TopicPattern::new("test.*").unwrap();
        let event_type = EventType::new("test.a.b.c.d.e").unwrap();
        assert!(pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_multi_wildcard_root() {
        let pattern = TopicPattern::new("*").unwrap();
        let event_type = EventType::new("anything.here").unwrap();
        assert!(pattern.matches(&event_type));
    }

    #[test]
    fn test_topic_pattern_empty_rejected() {
        let err = TopicPattern::new("").unwrap_err();
        assert!(matches!(err, EventError::InvalidPattern(_)));
    }

    #[test]
    fn test_topic_pattern_empty_segment_rejected() {
        let err = TopicPattern::new("test..event").unwrap_err();
        assert!(matches!(err, EventError::InvalidPattern(_)));
    }

    #[test]
    fn test_topic_pattern_wildcard_not_last_rejected() {
        let err = TopicPattern::new("test.*.event").unwrap_err();
        assert!(matches!(err, EventError::InvalidPattern(_)));
    }

    #[test]
    fn test_topic_pattern_canonical() {
        let pattern = TopicPattern::new("test.event.created").unwrap();
        assert_eq!(pattern.canonical, "test.event.created");
    }

    #[test]
    fn test_topic_pattern_segments_literal() {
        let pattern = TopicPattern::new("a.b.c").unwrap();
        assert_eq!(pattern.segments.len(), 3);
        assert!(matches!(pattern.segments[0], PatternSegment::Literal(ref s) if s == "a"));
        assert!(matches!(pattern.segments[1], PatternSegment::Literal(ref s) if s == "b"));
        assert!(matches!(pattern.segments[2], PatternSegment::Literal(ref s) if s == "c"));
    }

    #[test]
    fn test_topic_pattern_segments_wildcard() {
        let pattern = TopicPattern::new("a.+.c").unwrap();
        assert!(matches!(pattern.segments[0], PatternSegment::Literal(_)));
        assert!(matches!(pattern.segments[1], PatternSegment::SingleWildcard));
        assert!(matches!(pattern.segments[2], PatternSegment::Literal(_)));
    }

    #[test]
    fn test_topic_pattern_segments_multi_wildcard() {
        let pattern = TopicPattern::new("a.*").unwrap();
        assert!(matches!(pattern.segments[0], PatternSegment::Literal(_)));
        assert!(matches!(pattern.segments[1], PatternSegment::MultiWildcard));
    }

    #[test]
    fn test_filter_field_equals_true() {
        let expr = FilterExpr::FieldEquals {
            field: vec!["status".into()],
            value: serde_json::json!("active"),
        };
        let payload = serde_json::json!({"status": "active"});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_equals_false() {
        let expr = FilterExpr::FieldEquals {
            field: vec!["status".into()],
            value: serde_json::json!("active"),
        };
        let payload = serde_json::json!({"status": "inactive"});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_exists_true() {
        let expr = FilterExpr::FieldExists { field: vec!["name".into()] };
        let payload = serde_json::json!({"name": "test"});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_exists_false() {
        let expr = FilterExpr::FieldExists { field: vec!["missing".into()] };
        let payload = serde_json::json!({"name": "test"});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_in_true() {
        let expr = FilterExpr::FieldIn {
            field: vec!["color".into()],
            values: vec![serde_json::json!("red"), serde_json::json!("blue")],
        };
        let payload = serde_json::json!({"color": "blue"});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_in_false() {
        let expr = FilterExpr::FieldIn {
            field: vec!["color".into()],
            values: vec![serde_json::json!("red"), serde_json::json!("blue")],
        };
        let payload = serde_json::json!({"color": "green"});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_range_number_within() {
        let expr = FilterExpr::FieldRange {
            field: vec!["age".into()],
            min: serde_json::json!(10),
            max: serde_json::json!(20),
        };
        let payload = serde_json::json!({"age": 15});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_range_number_below() {
        let expr = FilterExpr::FieldRange {
            field: vec!["age".into()],
            min: serde_json::json!(10),
            max: serde_json::json!(20),
        };
        let payload = serde_json::json!({"age": 5});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_range_number_above() {
        let expr = FilterExpr::FieldRange {
            field: vec!["age".into()],
            min: serde_json::json!(10),
            max: serde_json::json!(20),
        };
        let payload = serde_json::json!({"age": 25});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_range_number_at_boundary() {
        let expr = FilterExpr::FieldRange {
            field: vec!["age".into()],
            min: serde_json::json!(10),
            max: serde_json::json!(20),
        };
        let payload_at_min = serde_json::json!({"age": 10});
        let payload_at_max = serde_json::json!({"age": 20});
        assert!(expr.evaluate(&payload_at_min));
        assert!(expr.evaluate(&payload_at_max));
    }

    #[test]
    fn test_filter_field_range_string() {
        let expr = FilterExpr::FieldRange {
            field: vec!["name".into()],
            min: serde_json::json!("aaa"),
            max: serde_json::json!("zzz"),
        };
        let payload = serde_json::json!({"name": "mmm"});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_not() {
        let inner = FilterExpr::FieldExists { field: vec!["missing".into()] };
        let expr = FilterExpr::Not(Box::new(inner));
        let payload = serde_json::json!({"name": "test"});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_and_all_true() {
        let expr = FilterExpr::And(vec![
            FilterExpr::FieldEquals { field: vec!["a".into()], value: serde_json::json!(1) },
            FilterExpr::FieldEquals { field: vec!["b".into()], value: serde_json::json!(2) },
        ]);
        let payload = serde_json::json!({"a": 1, "b": 2});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_and_one_false() {
        let expr = FilterExpr::And(vec![
            FilterExpr::FieldEquals { field: vec!["a".into()], value: serde_json::json!(1) },
            FilterExpr::FieldEquals { field: vec!["b".into()], value: serde_json::json!(99) },
        ]);
        let payload = serde_json::json!({"a": 1, "b": 2});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_or_one_true() {
        let expr = FilterExpr::Or(vec![
            FilterExpr::FieldEquals { field: vec!["a".into()], value: serde_json::json!(1) },
            FilterExpr::FieldEquals { field: vec!["a".into()], value: serde_json::json!(99) },
        ]);
        let payload = serde_json::json!({"a": 1});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_or_all_false() {
        let expr = FilterExpr::Or(vec![
            FilterExpr::FieldEquals { field: vec!["a".into()], value: serde_json::json!(99) },
            FilterExpr::FieldEquals { field: vec!["b".into()], value: serde_json::json!(98) },
        ]);
        let payload = serde_json::json!({"a": 1, "b": 2});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_field_missing_returns_false() {
        let expr = FilterExpr::FieldEquals {
            field: vec!["nonexistent".into()],
            value: serde_json::json!("anything"),
        };
        let payload = serde_json::json!({"a": 1});
        assert!(!expr.evaluate(&payload));
    }

    #[test]
    fn test_filter_deeply_nested_field() {
        let expr = FilterExpr::FieldEquals {
            field: vec!["a".into(), "b".into(), "c".into()],
            value: serde_json::json!(42),
        };
        let payload = serde_json::json!({"a": {"b": {"c": 42}}});
        assert!(expr.evaluate(&payload));
    }

    #[test]
    fn test_content_filter_evaluate_true() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .build(serde_json::to_vec(&serde_json::json!({"status": "ok"})).unwrap());
        let expr = FilterExpr::FieldEquals {
            field: vec!["status".into()],
            value: serde_json::json!("ok"),
        };
        let filter = ContentFilter { expression: expr };
        assert!(filter.evaluate(&event));
    }

    #[test]
    fn test_content_filter_evaluate_false() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .build(serde_json::to_vec(&serde_json::json!({"status": "fail"})).unwrap());
        let expr = FilterExpr::FieldEquals {
            field: vec!["status".into()],
            value: serde_json::json!("ok"),
        };
        let filter = ContentFilter { expression: expr };
        assert!(!filter.evaluate(&event));
    }

    #[test]
    fn test_content_filter_non_json_payload() {
        let event = EventBuilder::new("test.event")
            .unwrap()
            .build(vec![0, 1, 2, 3]);
        let expr = FilterExpr::FieldExists { field: vec!["x".into()] };
        let filter = ContentFilter { expression: expr };
        assert!(!filter.evaluate(&event));
    }

    #[test]
    fn test_subscriber_id_creation() {
        let sid = SubscriberId {
            id: "abc-123".into(),
            subsystem: Subsystem::Execution,
            name: "worker".into(),
        };
        assert_eq!(sid.id, "abc-123");
        assert_eq!(sid.subsystem, Subsystem::Execution);
        assert_eq!(sid.name, "worker");
    }

    #[test]
    fn test_delivery_guarantee_variants() {
        assert_ne!(DeliveryGuarantee::AtMostOnce as u8, DeliveryGuarantee::AtLeastOnce as u8);
        assert_ne!(DeliveryGuarantee::AtLeastOnce as u8, DeliveryGuarantee::ExactlyOnce as u8);
    }
}
