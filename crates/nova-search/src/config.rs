use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_limit")]
    pub default_limit: usize,
    #[serde(default = "default_max_limit")]
    pub max_limit: usize,
    #[serde(default = "default_bm25_k1")]
    pub bm25_k1: f64,
    #[serde(default = "default_bm25_b")]
    pub bm25_b: f64,
    #[serde(default = "default_fuzzy_max_distance")]
    pub fuzzy_max_distance: u8,
    #[serde(default = "default_highlight_snippet_len")]
    pub highlight_snippet_len: usize,
    #[serde(default = "default_highlight_max_snippets")]
    pub highlight_max_snippets: usize,
    #[serde(default = "default_refresh_interval_ms")]
    pub refresh_interval_ms: u64,
    #[serde(default = "default_merge_segment_threshold")]
    pub merge_segment_threshold: usize,
}

fn default_limit() -> usize { 10 }
fn default_max_limit() -> usize { 1000 }
fn default_bm25_k1() -> f64 { 1.2 }
fn default_bm25_b() -> f64 { 0.75 }
fn default_fuzzy_max_distance() -> u8 { 2 }
fn default_highlight_snippet_len() -> usize { 150 }
fn default_highlight_max_snippets() -> usize { 3 }
fn default_refresh_interval_ms() -> u64 { 1000 }
fn default_merge_segment_threshold() -> usize { 5 }

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_limit: default_limit(),
            max_limit: default_max_limit(),
            bm25_k1: default_bm25_k1(),
            bm25_b: default_bm25_b(),
            fuzzy_max_distance: default_fuzzy_max_distance(),
            highlight_snippet_len: default_highlight_snippet_len(),
            highlight_max_snippets: default_highlight_max_snippets(),
            refresh_interval_ms: default_refresh_interval_ms(),
            merge_segment_threshold: default_merge_segment_threshold(),
        }
    }
}
