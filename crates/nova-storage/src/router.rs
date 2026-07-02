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
