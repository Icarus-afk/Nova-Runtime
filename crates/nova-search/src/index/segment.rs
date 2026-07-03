use std::collections::HashMap;

use crate::document::IndexedDocument;
use crate::posting::list::PostingList;

#[derive(Debug, Clone)]
pub struct InMemorySegment {
    pub id: u64,
    pub inverted_index: HashMap<(String, String), PostingList>,
    pub stored_documents: HashMap<String, IndexedDocument>,
    pub doc_count: u64,
    pub total_tokens: u64,
    pub field_lengths: HashMap<String, HashMap<String, u64>>,
    pub field_values: HashMap<String, HashMap<String, String>>,
}

impl InMemorySegment {
    pub fn new(id: u64) -> Self {
        InMemorySegment {
            id,
            inverted_index: HashMap::new(),
            stored_documents: HashMap::new(),
            doc_count: 0,
            total_tokens: 0,
            field_lengths: HashMap::new(),
            field_values: HashMap::new(),
        }
    }

    pub fn all_text_fields(&self) -> Vec<&str> {
        let mut fields: Vec<&str> = self
            .inverted_index
            .keys()
            .map(|(f, _)| f.as_str())
            .collect();
        fields.sort();
        fields.dedup();
        fields
    }

    pub fn avg_field_length(&self, field: &str) -> f64 {
        if let Some(lengths) = self.field_lengths.get(field) {
            if lengths.is_empty() {
                return 0.0;
            }
            let total: u64 = lengths.values().sum();
            total as f64 / lengths.len() as f64
        } else {
            0.0
        }
    }

    pub fn field_length_for_doc(&self, field: &str, doc_id: &str) -> Option<u64> {
        self.field_lengths
            .get(field)
            .and_then(|lengths| lengths.get(doc_id))
            .copied()
    }

    pub fn doc_frequency(&self, field: &str, term: &str) -> u64 {
        self.inverted_index
            .get(&(field.to_string(), term.to_string()))
            .map(|pl| pl.len() as u64)
            .unwrap_or(0)
    }
}
