use std::sync::Arc;

use crate::analysis::AnalyzerPipeline;
use crate::config::SearchConfig;
use crate::document::IndexedDocument;
use crate::error::Result;
use crate::highlight::Highlighter;
use crate::index::writer::IndexWriter;
use crate::query::executor::QueryExecutor;
use crate::query::parser::QueryParser;
use crate::scoring::collector::ScoredDocument;
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct HighlightedDocument {
    pub doc_id: u64,
    pub score: f64,
    pub document: Option<IndexedDocument>,
    pub snippets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub hits: Vec<ScoredDocument>,
    pub total_hits: u64,
    pub search_time_ms: u64,
    pub max_score: f64,
}

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub num_docs: u64,
    pub num_terms: u64,
    pub field_count: usize,
}

pub struct SearchManager {
    writer: Arc<RwLock<IndexWriter>>,
    config: SearchConfig,
}

impl SearchManager {
    pub fn new() -> Self {
        let analyzer = AnalyzerPipeline::default();
        let writer = Arc::new(RwLock::new(IndexWriter::new(analyzer)));
        SearchManager {
            writer,
            config: SearchConfig::default(),
        }
    }

    pub fn with_config(config: SearchConfig) -> Self {
        let analyzer = AnalyzerPipeline::default();
        let writer = Arc::new(RwLock::new(IndexWriter::new(analyzer)));
        SearchManager { writer, config }
    }

    pub fn index_document(&self, doc: IndexedDocument) -> Result<()> {
        self.writer.write().add_document(doc)
    }

    pub fn delete_document(&self, doc_id: &str) -> Result<()> {
        self.writer.write().delete_document(doc_id)
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<ScoredDocument>> {
        let parsed = QueryParser::parse(query_str)?;
        let segment = self.writer.read().segment().clone();
        let executor = QueryExecutor::with_config(segment, self.config.bm25_k1, self.config.bm25_b);
        executor.execute(&parsed, limit)
    }

    pub fn search_with_pagination(
        &self,
        query_str: &str,
        limit: usize,
        search_after: Option<(f64, u64)>,
    ) -> Result<SearchResponse> {
        let start = std::time::Instant::now();
        let parsed = QueryParser::parse(query_str)?;
        let segment = self.writer.read().segment().clone();
        let executor = QueryExecutor::with_config(segment, self.config.bm25_k1, self.config.bm25_b);
        let mut results = executor.execute(&parsed, usize::MAX)?;

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.doc_id.cmp(&b.doc_id))
        });

        let total_hits = results.len() as u64;
        let max_score = results.first().map(|r| r.score).unwrap_or(0.0);

        let hits: Vec<ScoredDocument> = if let Some((after_score, after_id)) = search_after {
            results
                .into_iter()
                .filter(|r| r.score < after_score || (r.score == after_score && r.doc_id > after_id))
                .take(limit)
                .collect()
        } else {
            results.into_iter().take(limit).collect()
        };

        let elapsed = start.elapsed();
        Ok(SearchResponse {
            hits,
            total_hits,
            search_time_ms: elapsed.as_millis() as u64,
            max_score,
        })
    }

    pub fn search_with_highlight(
        &self,
        query_str: &str,
        limit: usize,
    ) -> Result<Vec<HighlightedDocument>> {
        let parsed = QueryParser::parse(query_str)?;
        let segment = self.writer.read().segment().clone();
        let executor = QueryExecutor::with_config(segment, self.config.bm25_k1, self.config.bm25_b);
        let results = executor.execute(&parsed, limit)?;

        let query_tokens = self.query_tokens(query_str);

        let highlighted = results
            .into_iter()
            .map(|sd| {
                let snippets = if let Some(ref doc) = sd.document {
                    let mut all_snippets = Vec::new();
                    for field in &doc.fields {
                        if let crate::document::FieldValue::Text(text) = &field.value {
                            let field_snippets =
                                Highlighter::highlight(text, &query_tokens, self.config.highlight_snippet_len);
                            all_snippets.extend(field_snippets);
                        }
                    }
                    all_snippets
                } else {
                    vec![]
                };

                HighlightedDocument {
                    doc_id: sd.doc_id,
                    score: sd.score,
                    document: sd.document,
                    snippets,
                }
            })
            .collect();

        Ok(highlighted)
    }

    pub fn search_faceted(
        &self,
        query_str: &str,
        facet_field: &str,
        limit: usize,
    ) -> Result<crate::facet::FacetResult> {
        let parsed = QueryParser::parse(query_str)?;
        let segment = self.writer.read().segment().clone();
        let executor = QueryExecutor::with_config(segment, self.config.bm25_k1, self.config.bm25_b);
        executor.execute_faceted(&parsed, facet_field, limit)
    }

    pub fn refresh(&self) -> Result<()> {
        let analyzer = AnalyzerPipeline::default();
        let new_writer = IndexWriter::new(analyzer);
        *self.writer.write() = new_writer;
        Ok(())
    }

    pub fn update_document(&self, doc_id: &str, doc: IndexedDocument) -> Result<()> {
        let mut writer = self.writer.write();
        if writer.segment().stored_documents.contains_key(doc_id) {
            writer.delete_document(doc_id)?;
        }
        let mut updated_doc = doc;
        updated_doc.id = doc_id.to_string();
        writer.add_document(updated_doc)?;
        Ok(())
    }

    pub fn stats(&self) -> IndexStats {
        let segment = self.writer.read().segment().clone();
        let field_count: usize = segment
            .inverted_index
            .keys()
            .map(|(f, _)| f.clone())
            .collect::<std::collections::HashSet<_>>()
            .len();
        IndexStats {
            num_docs: segment.doc_count,
            num_terms: segment.total_tokens,
            field_count,
        }
    }

    fn query_tokens(&self, query_str: &str) -> Vec<crate::analysis::tokenizer::Token> {
        let parser = AnalyzerPipeline::default();
        parser.analyze_query(query_str)
    }
}

impl Default for SearchManager {
    fn default() -> Self {
        SearchManager::new()
    }
}
