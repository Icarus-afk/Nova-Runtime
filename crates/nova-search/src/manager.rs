use crate::analysis::AnalyzerPipeline;
use crate::config::SearchConfig;
use crate::document::IndexedDocument;
use crate::error::Result;
use crate::highlight::Highlighter;
use crate::index::writer::IndexWriter;
use crate::query::executor::QueryExecutor;
use crate::query::parser::QueryParser;
use crate::scoring::collector::ScoredDocument;

#[derive(Debug, Clone)]
pub struct HighlightedDocument {
    pub doc_id: u64,
    pub score: f64,
    pub document: Option<IndexedDocument>,
    pub snippets: Vec<String>,
}

pub struct SearchManager {
    writer: IndexWriter,
    config: SearchConfig,
}

impl SearchManager {
    pub fn new() -> Self {
        let analyzer = AnalyzerPipeline::default();
        let writer = IndexWriter::new(analyzer);
        SearchManager {
            writer,
            config: SearchConfig::default(),
        }
    }

    pub fn with_config(config: SearchConfig) -> Self {
        let analyzer = AnalyzerPipeline::default();
        let writer = IndexWriter::new(analyzer);
        SearchManager { writer, config }
    }

    pub fn index_document(&mut self, doc: IndexedDocument) -> Result<()> {
        self.writer.add_document(doc)
    }

    pub fn delete_document(&mut self, doc_id: &str) -> Result<()> {
        self.writer.delete_document(doc_id)
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<ScoredDocument>> {
        let parsed = QueryParser::parse(query_str)?;
        let segment = self.writer.segment().clone();
        let executor = QueryExecutor::new(segment);
        executor.execute(&parsed, limit)
    }

    pub fn search_with_highlight(
        &self,
        query_str: &str,
        limit: usize,
    ) -> Result<Vec<HighlightedDocument>> {
        let parsed = QueryParser::parse(query_str)?;
        let segment = self.writer.segment().clone();
        let executor = QueryExecutor::new(segment);
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
        let segment = self.writer.segment().clone();
        let executor = QueryExecutor::new(segment);
        executor.execute_faceted(&parsed, facet_field, limit)
    }

    pub fn refresh(&mut self) -> Result<()> {
        let analyzer = AnalyzerPipeline::default();
        let new_writer = IndexWriter::new(analyzer);
        let _ = std::mem::replace(&mut self.writer, new_writer);
        Ok(())
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
