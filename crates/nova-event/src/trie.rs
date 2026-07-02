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
