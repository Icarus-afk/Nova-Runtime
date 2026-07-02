use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use uuid::Uuid;
use crate::{Subscription, TopicPattern, PatternSegment, EventType, Result};

struct TrieNode {
    literal_children: RwLock<HashMap<String, Arc<TrieNode>>>,
    single_wildcard: RwLock<Option<Arc<TrieNode>>>,
    multi_wildcard: RwLock<Option<Arc<TrieNode>>>,
    subscriptions: RwLock<Vec<Subscription>>,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            literal_children: RwLock::new(HashMap::new()),
            single_wildcard: RwLock::new(None),
            multi_wildcard: RwLock::new(None),
            subscriptions: RwLock::new(Vec::new()),
        }
    }
}

pub struct SubscriptionTrie {
    root: Arc<TrieNode>,
}

impl SubscriptionTrie {
    pub fn new() -> Self {
        SubscriptionTrie {
            root: Arc::new(TrieNode::new()),
        }
    }

    pub fn insert(&self, pattern: &TopicPattern, sub: Subscription) -> Result<()> {
        let mut current = Arc::clone(&self.root);
        for segment in &pattern.segments {
            current = match segment {
                PatternSegment::Literal(lit) => {
                    let mut children = current.literal_children.write();
                    let entry = children.entry(lit.clone())
                        .or_insert_with(|| Arc::new(TrieNode::new()));
                    Arc::clone(entry)
                }
                PatternSegment::SingleWildcard => {
                    let mut sw = current.single_wildcard.write();
                    let node = sw.get_or_insert_with(|| Arc::new(TrieNode::new()));
                    Arc::clone(node)
                }
                PatternSegment::MultiWildcard => {
                    let mut mw = current.multi_wildcard.write();
                    let node = mw.get_or_insert_with(|| Arc::new(TrieNode::new()));
                    Arc::clone(node)
                }
            };
        }
        current.subscriptions.write().push(sub);
        Ok(())
    }

    pub fn remove(&self, sub_id: Uuid) -> bool {
        self.remove_recursive(&self.root, sub_id)
    }

    fn remove_recursive(&self, node: &Arc<TrieNode>, sub_id: Uuid) -> bool {
        {
            let mut subs = node.subscriptions.write();
            if let Some(pos) = subs.iter().position(|s| s.id == sub_id) {
                subs.remove(pos);
                return true;
            }
        }

        for child in node.literal_children.read().values() {
            if self.remove_recursive(child, sub_id) {
                return true;
            }
        }

        if let Some(ref child) = *node.single_wildcard.read() {
            if self.remove_recursive(child, sub_id) {
                return true;
            }
        }

        if let Some(ref child) = *node.multi_wildcard.read() {
            if self.remove_recursive(child, sub_id) {
                return true;
            }
        }

        false
    }

    pub fn lookup(&self, event_type: &EventType) -> Vec<Subscription> {
        let mut results = Vec::new();
        self.lookup_recursive(&self.root, &event_type.segments, &mut results);
        results
    }

    fn lookup_recursive(
        &self,
        node: &Arc<TrieNode>,
        segments: &[String],
        results: &mut Vec<Subscription>,
    ) {
        if segments.is_empty() {
            results.extend(node.subscriptions.read().iter().cloned());
        }

        {
            let mw_guard = node.multi_wildcard.read();
            if let Some(ref mw_node) = *mw_guard {
                results.extend(mw_node.subscriptions.read().iter().cloned());
            }
        }

        if segments.is_empty() {
            return;
        }

        let first = &segments[0];
        let rest = &segments[1..];

        if let Some(child) = node.literal_children.read().get(first).cloned() {
            self.lookup_recursive(&child, rest, results);
        }

        {
            let sw_guard = node.single_wildcard.read();
            if let Some(ref child) = *sw_guard {
                self.lookup_recursive(child, rest, results);
            }
        }
    }

    pub fn all_subscriptions(&self) -> Vec<Subscription> {
        let mut subs = Vec::new();
        self.collect_subscriptions(&self.root, &mut subs);
        subs
    }

    fn collect_subscriptions(&self, node: &Arc<TrieNode>, subs: &mut Vec<Subscription>) {
        subs.extend(node.subscriptions.read().iter().cloned());
        for child in node.literal_children.read().values() {
            self.collect_subscriptions(child, subs);
        }
        if let Some(ref child) = *node.single_wildcard.read() {
            self.collect_subscriptions(child, subs);
        }
        if let Some(ref child) = *node.multi_wildcard.read() {
            self.collect_subscriptions(child, subs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventType, SubscriberId, Subsystem, DeliveryGuarantee};
    use crossbeam::channel;

    fn make_sub(topic: &str) -> Subscription {
        let (tx, _rx) = channel::bounded(16);
        Subscription {
            id: Uuid::new_v4(),
            subscriber: SubscriberId {
                id: "test".into(),
                subsystem: Subsystem::System,
                name: "tester".into(),
            },
            topic: TopicPattern::new(topic).unwrap(),
            content_filter: None,
            delivery_guarantee: DeliveryGuarantee::AtMostOnce,
            max_retries: 0,
            retry_backoff_ms: 0,
            max_backoff_ms: 0,
            queue_capacity: 16,
            created_at: 0,
            active: true,
            consumer_group: None,
            sender: tx,
        }
    }

    fn event_type(s: &str) -> EventType {
        EventType::new(s).unwrap()
    }

    #[test]
    fn test_empty_trie_lookup() {
        let trie = SubscriptionTrie::new();
        let results = trie.lookup(&event_type("test.event"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_insert_and_lookup_literal() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.event.created");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event.created"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, sub.id);
    }

    #[test]
    fn test_literal_no_match() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.event.created");
        let topic = sub.topic.clone();
        trie.insert(&topic, sub).unwrap();
        let results = trie.lookup(&event_type("test.event.deleted"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_single_wildcard_matching() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.+.created");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event.created"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, sub.id);
    }

    #[test]
    fn test_single_wildcard_no_match_different_length() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.+.created");
        let topic = sub.topic.clone();
        trie.insert(&topic, sub).unwrap();
        let results = trie.lookup(&event_type("test.event.extra.created"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_multi_wildcard_matching() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.*");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event.created"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, sub.id);
    }

    #[test]
    fn test_multi_wildcard_matches_single_segment() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.*");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_multi_wildcard_matches_many() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.*");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        let results = trie.lookup(&event_type("test.a.b.c.d.e"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_multi_wildcard_at_root() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("*");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        let results = trie.lookup(&event_type("anything.here"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_remove_subscription() {
        let trie = SubscriptionTrie::new();
        let sub = make_sub("test.event");
        trie.insert(&sub.topic, sub.clone()).unwrap();
        assert_eq!(trie.lookup(&event_type("test.event")).len(), 1);
        assert!(trie.remove(sub.id));
        assert_eq!(trie.lookup(&event_type("test.event")).len(), 0);
    }

    #[test]
    fn test_remove_nonexistent() {
        let trie = SubscriptionTrie::new();
        assert!(!trie.remove(Uuid::new_v4()));
    }

    #[test]
    fn test_all_subscriptions_empty() {
        let trie = SubscriptionTrie::new();
        assert!(trie.all_subscriptions().is_empty());
    }

    #[test]
    fn test_all_subscriptions_returns_all() {
        let trie = SubscriptionTrie::new();
        let sub1 = make_sub("test.event.one");
        let sub2 = make_sub("test.event.two");
        let sub3 = make_sub("other.*");
        trie.insert(&sub1.topic, sub1.clone()).unwrap();
        trie.insert(&sub2.topic, sub2.clone()).unwrap();
        trie.insert(&sub3.topic, sub3.clone()).unwrap();
        assert_eq!(trie.all_subscriptions().len(), 3);
    }

    #[test]
    fn test_multiple_subscriptions_same_topic() {
        let trie = SubscriptionTrie::new();
        let sub1 = make_sub("test.event");
        let sub2 = make_sub("test.event");
        trie.insert(&sub1.topic, sub1.clone()).unwrap();
        trie.insert(&sub2.topic, sub2.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event"));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_literal_and_wildcard_both_match() {
        let trie = SubscriptionTrie::new();
        let sub1 = make_sub("test.event");
        let sub2 = make_sub("test.+");
        trie.insert(&sub1.topic, sub1.clone()).unwrap();
        trie.insert(&sub2.topic, sub2.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event"));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_remove_only_removes_one() {
        let trie = SubscriptionTrie::new();
        let sub1 = make_sub("test.event");
        let sub2 = make_sub("test.event");
        trie.insert(&sub1.topic, sub1.clone()).unwrap();
        trie.insert(&sub2.topic, sub2.clone()).unwrap();
        assert!(trie.remove(sub1.id));
        let results = trie.lookup(&event_type("test.event"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, sub2.id);
    }

    #[test]
    fn test_remove_after_second_insert() {
        let trie = SubscriptionTrie::new();
        let sub1 = make_sub("test.event");
        trie.insert(&sub1.topic, sub1.clone()).unwrap();
        assert!(trie.remove(sub1.id));
        let sub2 = make_sub("test.event");
        trie.insert(&sub2.topic, sub2.clone()).unwrap();
        let results = trie.lookup(&event_type("test.event"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, sub2.id);
    }
}
