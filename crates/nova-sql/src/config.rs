use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SQLConfig {
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,
    #[serde(default = "default_max_columns")]
    pub max_columns: usize,
    #[serde(default = "default_sql_limit")]
    pub default_limit: usize,
}

fn default_max_batch_size() -> usize { 1024 }
fn default_max_columns() -> usize { 256 }
fn default_sql_limit() -> usize { 1000 }

impl Default for SQLConfig {
    fn default() -> Self {
        Self {
            max_batch_size: default_max_batch_size(),
            max_columns: default_max_columns(),
            default_limit: default_sql_limit(),
        }
    }
}
