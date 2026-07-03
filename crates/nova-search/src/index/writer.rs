use std::collections::HashMap;

use crate::analysis::AnalyzerPipeline;
use crate::document::{FieldValue, IndexedDocument};
use crate::error::{Result, SearchError};
use crate::index::segment::InMemorySegment;
use crate::posting::list::PostingEntry;

pub struct IndexWriter {
    segment: InMemorySegment,
    analyzer: AnalyzerPipeline,
    next_doc_id: u64,
}

impl IndexWriter {
    pub fn new(analyzer: AnalyzerPipeline) -> Self {
        IndexWriter {
            segment: InMemorySegment::new(0),
            analyzer,
            next_doc_id: 1,
        }
    }

    pub fn segment(&self) -> &InMemorySegment {
        &self.segment
    }

    pub fn add_document(&mut self, doc: IndexedDocument) -> Result<()> {
        let doc_id = self.next_doc_id;
        self.next_doc_id += 1;

        for field in &doc.fields {
            match &field.value {
                FieldValue::Text(text) => {
                    let tokens = self.analyzer.analyze(text);
                    let field_name = field.name.clone();
                    let doc_id_str = doc_id.to_string();

                    let field_len = self.segment
                        .field_lengths
                        .entry(field_name.clone())
                        .or_default();
                    field_len.insert(doc_id_str.clone(), tokens.len() as u64);

                    self.segment.total_tokens += tokens.len() as u64;

                    let mut term_positions: HashMap<String, Vec<u32>> = HashMap::new();
                    for token in &tokens {
                        term_positions
                            .entry(token.term.clone())
                            .or_default()
                            .push(token.position as u32);
                    }

                    for (term, positions) in term_positions {
                        let entry = PostingEntry {
                            doc_id,
                            term_frequency: positions.len() as u32,
                            positions,
                        };
                        let key = (field_name.clone(), term);
                        let posting_list = self.segment.inverted_index.entry(key).or_default();
                        posting_list.push(entry);
                    }
                }
                FieldValue::Integer(n) => {
                    let doc_id_str = doc_id.to_string();
                    let field_name = field.name.clone();
                    self.segment
                        .field_values
                        .entry(field_name)
                        .or_default()
                        .insert(doc_id_str, n.to_string());
                }
                FieldValue::Float(f) => {
                    let doc_id_str = doc_id.to_string();
                    let field_name = field.name.clone();
                    self.segment
                        .field_values
                        .entry(field_name)
                        .or_default()
                        .insert(doc_id_str, f.to_string());
                }
                FieldValue::Bool(b) => {
                    let doc_id_str = doc_id.to_string();
                    let field_name = field.name.clone();
                    self.segment
                        .field_values
                        .entry(field_name)
                        .or_default()
                        .insert(doc_id_str, b.to_string());
                }
            }
        }

        self.segment.stored_documents.insert(doc_id.to_string(), doc);
        self.segment.doc_count += 1;

        Ok(())
    }

    pub fn delete_document(&mut self, doc_id: &str) -> Result<()> {
        let removed = self.segment.stored_documents.remove(doc_id);
        if removed.is_none() {
            return Err(SearchError::IndexNotFound(doc_id.to_string()));
        }

        self.segment.inverted_index.retain(|_, postings| {
            postings.retain(|entry| entry.doc_id.to_string() != doc_id);
            !postings.is_empty()
        });

        for lengths in self.segment.field_lengths.values_mut() {
            lengths.remove(doc_id);
        }

        for values in self.segment.field_values.values_mut() {
            values.remove(doc_id);
        }

        self.segment.doc_count = self.segment.doc_count.saturating_sub(1);

        Ok(())
    }

    pub fn finish(self) -> InMemorySegment {
        self.segment
    }
}
