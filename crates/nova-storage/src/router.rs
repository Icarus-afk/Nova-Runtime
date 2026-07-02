use nova_core::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineType {
    BTree,
    LSM,
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub prefix: Vec<u8>,
    pub target: EngineType,
    pub min_size: u16,
    pub max_size: u16,
    pub priority: u8,
}

#[derive(Debug, Clone)]
pub struct KeyRouter {
    pub rules: Vec<RoutingRule>,
}

impl KeyRouter {
    pub fn new() -> Self {
        KeyRouter { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: RoutingRule) {
        self.rules.push(rule);
    }

    pub fn route(&self, key: &Key) -> EngineType {
        let mut best: Option<&RoutingRule> = None;
        for rule in &self.rules {
            if key.len() < rule.min_size as usize {
                continue;
            }
            if rule.max_size > 0 && key.len() > rule.max_size as usize {
                continue;
            }
            if !key.as_bytes().starts_with(&rule.prefix) {
                continue;
            }
            match best {
                None => best = Some(rule),
                Some(current) => {
                    if rule.priority > current.priority {
                        best = Some(rule);
                    }
                }
            }
        }
        best.map(|r| r.target).unwrap_or(EngineType::BTree)
    }

    pub fn default() -> Self {
        let mut router = KeyRouter::new();
        let btree_prefixes = ["meta:", "schema:", "auth:", "sql:", "index:", "blob:"];
        let lsm_prefixes = ["queue:", "event:", "log:", "cache:", "search:", "ts:", "session:"];
        for p in &btree_prefixes {
            router.add_rule(RoutingRule {
                prefix: p.as_bytes().to_vec(),
                target: EngineType::BTree,
                min_size: 0,
                max_size: 0,
                priority: 1,
            });
        }
        for p in &lsm_prefixes {
            router.add_rule(RoutingRule {
                prefix: p.as_bytes().to_vec(),
                target: EngineType::LSM,
                min_size: 0,
                max_size: 0,
                priority: 1,
            });
        }
        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_router_new_empty() {
        let router = KeyRouter::new();
        assert!(router.rules.is_empty());
    }

    #[test]
    fn test_key_router_default_has_rules() {
        let router = KeyRouter::default();
        assert!(!router.rules.is_empty());
        assert_eq!(router.rules.len(), 13);
    }

    #[test]
    fn test_route_btree_prefix() {
        let router = KeyRouter::default();
        assert_eq!(router.route(&Key::from("meta:user:123")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("schema:tables")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("auth:token")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("sql:query")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("index:idx1")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("blob:data")), EngineType::BTree);
    }

    #[test]
    fn test_route_lsm_prefix() {
        let router = KeyRouter::default();
        assert_eq!(router.route(&Key::from("queue:task")), EngineType::LSM);
        assert_eq!(router.route(&Key::from("event:click")), EngineType::LSM);
        assert_eq!(router.route(&Key::from("log:error")), EngineType::LSM);
        assert_eq!(router.route(&Key::from("cache:item")), EngineType::LSM);
        assert_eq!(router.route(&Key::from("search:q")), EngineType::LSM);
        assert_eq!(router.route(&Key::from("ts:12345")), EngineType::LSM);
        assert_eq!(router.route(&Key::from("session:s1")), EngineType::LSM);
    }

    #[test]
    fn test_route_fallback_to_btree() {
        let router = KeyRouter::default();
        assert_eq!(router.route(&Key::from("unknown:prefix")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("random_key")), EngineType::BTree);
        assert_eq!(router.route(&Key::from("")), EngineType::BTree);
    }

    #[test]
    fn test_route_custom_rule() {
        let mut router = KeyRouter::new();
        router.add_rule(RoutingRule {
            prefix: b"custom:".to_vec(),
            target: EngineType::LSM,
            min_size: 0,
            max_size: 0,
            priority: 1,
        });
        assert_eq!(router.route(&Key::from("custom:data")), EngineType::LSM);
    }

    #[test]
    fn test_route_prefix_mismatch_falls_back() {
        let mut router = KeyRouter::new();
        router.add_rule(RoutingRule {
            prefix: b"abc:".to_vec(),
            target: EngineType::LSM,
            min_size: 0,
            max_size: 0,
            priority: 1,
        });
        assert_eq!(router.route(&Key::from("xyz:data")), EngineType::BTree);
    }

    #[test]
    fn test_route_size_bounds_min() {
        let mut router = KeyRouter::new();
        router.add_rule(RoutingRule {
            prefix: b"data:".to_vec(),
            target: EngineType::LSM,
            min_size: 7,
            max_size: 0,
            priority: 1,
        });
        // "data:ab" length 7 — meets min_size
        assert_eq!(router.route(&Key::from("data:ab")), EngineType::LSM);
        // "data:a" length 6 — below min_size
        assert_eq!(router.route(&Key::from("data:a")), EngineType::BTree);
    }

    #[test]
    fn test_route_size_bounds_max() {
        let mut router = KeyRouter::new();
        router.add_rule(RoutingRule {
            prefix: b"data:".to_vec(),
            target: EngineType::LSM,
            min_size: 0,
            max_size: 10,
            priority: 1,
        });
        // "data:abcde" length 10 — within max_size
        assert_eq!(router.route(&Key::from("data:abcde")), EngineType::LSM);
        // "data:abcdef" length 11 — exceeds max_size
        assert_eq!(router.route(&Key::from("data:abcdef")), EngineType::BTree);
    }

    #[test]
    fn test_route_priority_wins() {
        let mut router = KeyRouter::new();
        router.add_rule(RoutingRule {
            prefix: b"shared:".to_vec(),
            target: EngineType::BTree,
            min_size: 0,
            max_size: 0,
            priority: 1,
        });
        router.add_rule(RoutingRule {
            prefix: b"shared:".to_vec(),
            target: EngineType::LSM,
            min_size: 0,
            max_size: 0,
            priority: 10,
        });
        // Higher priority (10) should win
        assert_eq!(router.route(&Key::from("shared:key")), EngineType::LSM);
    }

    #[test]
    fn test_route_rule_without_prefix_bare() {
        let mut router = KeyRouter::new();
        router.add_rule(RoutingRule {
            prefix: vec![],
            target: EngineType::LSM,
            min_size: 0,
            max_size: 0,
            priority: 1,
        });
        // Empty prefix matches everything
        assert_eq!(router.route(&Key::from("anything")), EngineType::LSM);
    }
}
