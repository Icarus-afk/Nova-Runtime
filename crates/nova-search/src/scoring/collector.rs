use crate::document::IndexedDocument;

#[derive(Debug, Clone)]
pub struct ScoredDocument {
    pub doc_id: u64,
    pub score: f64,
    pub document: Option<IndexedDocument>,
}

pub struct TopDocs;

impl TopDocs {
    pub fn collect(mut results: Vec<ScoredDocument>, limit: usize) -> Vec<ScoredDocument> {
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }
}
