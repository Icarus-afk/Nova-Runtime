use std::collections::{HashMap, HashSet};

use super::ast::{BoolOperator, Query};
use crate::analysis::tokenizer::StandardTokenizer;
use crate::analysis::stemmer::PorterStemmer;
use crate::error::Result;
use crate::facet::FacetResult;
use crate::fuzzy::levenshtein::find_fuzzy_matches;
use crate::index::segment::InMemorySegment;
use crate::scoring::bm25::BM25Scorer;
use crate::scoring::collector::ScoredDocument;

pub struct QueryExecutor {
    segment: InMemorySegment,
    scorer: BM25Scorer,
}

impl QueryExecutor {
    pub fn new(segment: InMemorySegment) -> Self {
        let scorer = BM25Scorer::new(&segment);
        QueryExecutor { segment, scorer }
    }

    pub fn segment(&self) -> &InMemorySegment {
        &self.segment
    }

    pub fn execute(&self, query: &Query, limit: usize) -> Result<Vec<ScoredDocument>> {
        let mut doc_scores: HashMap<u64, f64> = HashMap::new();
        self.collect_scores(query, &mut doc_scores, 1.0)?;

        let mut results: Vec<ScoredDocument> = doc_scores
            .into_iter()
            .filter(|(doc_id, score)| *score > 0.0 || self.doc_exists(*doc_id))
            .map(|(doc_id, score)| {
                let stored = self.segment.stored_documents.get(&doc_id.to_string()).cloned();
                ScoredDocument {
                    doc_id,
                    score,
                    document: stored,
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    }

    fn doc_exists(&self, doc_id: u64) -> bool {
        self.segment.stored_documents.contains_key(&doc_id.to_string())
    }

    pub fn execute_faceted(&self, query: &Query, facet_field: &str, limit: usize) -> Result<FacetResult> {
        let results = self.execute(query, usize::MAX)?;
        let mut counts: HashMap<String, usize> = HashMap::new();

        for result in &results {
            if let Some(doc) = &result.document {
                if let Some(val) = doc.text_value(facet_field) {
                    *counts.entry(val.to_string()).or_insert(0) += 1;
                }
            }
        }

        let mut entries: Vec<(String, usize)> = counts.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(limit);

        Ok(FacetResult {
            field: facet_field.to_string(),
            entries,
            total_docs: results.len(),
        })
    }

    fn collect_scores(&self, query: &Query, scores: &mut HashMap<u64, f64>, boost: f64) -> Result<()> {
        match query {
            Query::Term { field, value } => {
                let fields: Vec<&str> = if let Some(f) = field {
                    vec![f.as_str()]
                } else {
                    self.segment.all_text_fields()
                };
                let stemmed = PorterStemmer::stem(value);
                for f in &fields {
                    if let Some(postings) = self.segment.inverted_index.get(&(f.to_string(), stemmed.clone())) {
                        let avg_field_len = self.segment.avg_field_length(f);
                        let total_docs = self.segment.doc_count;
                        for entry in postings {
                            let idf = BM25Scorer::idf(total_docs, postings.len() as u64);
                            let field_len = self.segment.field_length_for_doc(f, &entry.doc_id.to_string()).unwrap_or(0);
                            let tf_score = self.scorer.tf_score(
                                entry.term_frequency as f64,
                                field_len as f64,
                                avg_field_len,
                            );
                            let score = idf * tf_score * boost * 1.0;
                            *scores.entry(entry.doc_id).or_insert(0.0) += score;
                        }
                    }
                }
            }
            Query::Phrase { field, value, slop: _ } => {
                let stemmed_terms: Vec<String> = StandardTokenizer::tokenize(value)
                    .into_iter()
                    .map(|t| PorterStemmer::stem(&t.term))
                    .collect();

                if stemmed_terms.is_empty() {
                    return Ok(());
                }

                let fields: Vec<&str> = if let Some(f) = field {
                    vec![f.as_str()]
                } else {
                    self.segment.all_text_fields()
                };

                for f in &fields {
                    let mut phrase_matches: HashMap<u64, u32> = HashMap::new();

                    let first_term = &stemmed_terms[0];
                    let first_postings = self.segment.inverted_index.get(&(f.to_string(), first_term.clone()));

                    if let Some(postings) = first_postings {
                        for entry in postings {
                            let mut positions_for_term: HashMap<usize, Vec<u32>> = HashMap::new();
                            positions_for_term.insert(0, entry.positions.clone());

                            let mut matched = true;
                            for (i, term) in stemmed_terms.iter().enumerate().skip(1) {
                                let term_postings = self.segment.inverted_index.get(&(f.to_string(), term.clone()));
                                if let Some(tp) = term_postings {
                                    if let Some(te) = tp.iter().find(|te| te.doc_id == entry.doc_id) {
                                        positions_for_term.insert(i, te.positions.clone());
                                    } else {
                                        matched = false;
                                        break;
                                    }
                                } else {
                                    matched = false;
                                    break;
                                }
                            }

                            if matched {
                                let pos_lists: Vec<&Vec<u32>> = (0..stemmed_terms.len())
                                    .filter_map(|i| positions_for_term.get(&i))
                                    .collect();

                                if !pos_lists.is_empty() && Self::consecutive_positions(&pos_lists) {
                                    let total_tf: u32 = pos_lists.iter().map(|p| p.len() as u32).sum();
                                    *phrase_matches.entry(entry.doc_id).or_insert(0) += total_tf;
                                }
                            }
                        }
                    }

                    let avg_field_len = self.segment.avg_field_length(f);
                    let total_docs = self.segment.doc_count;
                    for (doc_id, tf) in phrase_matches {
                        let field_len = self.segment.field_length_for_doc(f, &doc_id.to_string()).unwrap_or(0);
                        let idf = BM25Scorer::idf(total_docs, 1);
                        let tf_score = self.scorer.tf_score(tf as f64, field_len as f64, avg_field_len);
                        *scores.entry(doc_id).or_insert(0.0) += idf * tf_score * boost;
                    }
                }
            }
            Query::Prefix { field, value } => {
                let fields: Vec<&str> = if let Some(f) = field {
                    vec![f.as_str()]
                } else {
                    self.segment.all_text_fields()
                };
                let stemmed = PorterStemmer::stem(value);
                for f in &fields {
                    let prefix = (f.to_string(), stemmed.clone());
                    for ((field_name, term), postings) in &self.segment.inverted_index {
                        if field_name != f {
                            continue;
                        }
                        if term.starts_with(&prefix.1) {
                            let avg_field_len = self.segment.avg_field_length(f);
                            let total_docs = self.segment.doc_count;
                            for entry in postings {
                                let idf = BM25Scorer::idf(total_docs, postings.len() as u64);
                                let field_len = self
                                    .segment
                                    .field_length_for_doc(f, &entry.doc_id.to_string())
                                    .unwrap_or(0);
                                let tf_score = self.scorer.tf_score(
                                    entry.term_frequency as f64,
                                    field_len as f64,
                                    avg_field_len,
                                );
                                *scores.entry(entry.doc_id).or_insert(0.0) += idf * tf_score * boost;
                            }
                        }
                    }
                }
            }
            Query::Fuzzy { field, value, max_distance } => {
                let fields: Vec<&str> = if let Some(f) = field {
                    vec![f.as_str()]
                } else {
                    self.segment.all_text_fields()
                };
                let stemmed = PorterStemmer::stem(value);
                for f in &fields {
                    let candidates: Vec<String> = self
                        .segment
                        .inverted_index
                        .keys()
                        .filter(|(fn_, _)| fn_ == f)
                        .map(|(_, t)| t.clone())
                        .collect();
                    let matches = find_fuzzy_matches(&stemmed, &candidates, *max_distance);
                    let avg_field_len = self.segment.avg_field_length(f);
                    let total_docs = self.segment.doc_count;
                    for m in &matches {
                        if let Some(postings) = self.segment.inverted_index.get(&(f.to_string(), m.clone())) {
                            for entry in postings {
                                let idf = BM25Scorer::idf(total_docs, postings.len() as u64);
                                let field_len = self
                                    .segment
                                    .field_length_for_doc(f, &entry.doc_id.to_string())
                                    .unwrap_or(0);
                                let tf_score = self.scorer.tf_score(
                                    entry.term_frequency as f64,
                                    field_len as f64,
                                    avg_field_len,
                                );
                                *scores.entry(entry.doc_id).or_insert(0.0) += idf * tf_score * boost;
                            }
                        }
                    }
                }
            }
            Query::Range { field, lower, upper, inclusive } => {
                if let Some(postings) = self.segment.field_values.get(field) {
                    for (doc_id_str, val) in postings {
                        let doc_id: u64 = doc_id_str.parse().unwrap_or(0);
                        if Self::in_range(val, lower, upper, *inclusive) {
                            *scores.entry(doc_id).or_insert(0.0) += boost;
                        }
                    }
                }
            }
            Query::Bool { operator, clauses } => match operator {
                BoolOperator::And => {
                    if clauses.is_empty() {
                        return Ok(());
                    }
                    let mut positive_scores: Vec<HashMap<u64, f64>> = Vec::new();
                    let mut negative_scores: Vec<HashMap<u64, f64>> = Vec::new();
                    for clause in clauses {
                        let mut sub_scores = HashMap::new();
                        self.collect_scores(clause, &mut sub_scores, boost)?;
                        match clause {
                            Query::Bool { operator: BoolOperator::Not, .. } => {
                                negative_scores.push(sub_scores);
                            }
                            _ => {
                                positive_scores.push(sub_scores);
                            }
                        }
                    }
                    if positive_scores.is_empty() {
                        return Ok(());
                    }
                    let mut result_set: HashSet<u64> = positive_scores[0].keys().cloned().collect();
                    for sub in &positive_scores[1..] {
                        let keys: HashSet<u64> = sub.keys().cloned().collect();
                        result_set = result_set.intersection(&keys).cloned().collect();
                    }
                    for excluded in &negative_scores {
                        let excluded_keys: HashSet<u64> = excluded.keys().cloned().collect();
                        result_set = result_set.difference(&excluded_keys).cloned().collect();
                    }
                    for doc_id in result_set {
                        let total: f64 = positive_scores.iter().filter_map(|s| s.get(&doc_id)).sum();
                        scores.insert(doc_id, total);
                    }
                }
                BoolOperator::Or => {
                    for clause in clauses {
                        self.collect_scores(clause, scores, boost)?;
                    }
                }
                BoolOperator::Not => {
                    if let Some(clause) = clauses.first() {
                        let mut excluded = HashMap::new();
                        self.collect_scores(clause, &mut excluded, boost)?;
                        for (doc_id, score) in excluded {
                            scores.insert(doc_id, score);
                        }
                    }
                }
            },
            Query::MatchAll => {
                for (doc_id_str, _doc) in &self.segment.stored_documents {
                    let doc_id: u64 = doc_id_str.parse().unwrap_or(0);
                    scores.insert(doc_id, 1.0);
                }
            }
        }
        Ok(())
    }

    fn consecutive_positions(pos_lists: &[&Vec<u32>]) -> bool {
        if pos_lists.is_empty() {
            return true;
        }
        for &start_pos in pos_lists[0] {
            let mut found = true;
            for (i, list) in pos_lists.iter().enumerate().skip(1) {
                if !list.contains(&(start_pos + i as u32)) {
                    found = false;
                    break;
                }
            }
            if found {
                return true;
            }
        }
        false
    }

    fn in_range(val: &str, lower: &str, upper: &str, inclusive: bool) -> bool {
        if let (Ok(v), Ok(l), Ok(u)) = (val.parse::<f64>(), lower.parse::<f64>(), upper.parse::<f64>()) {
            let below = if inclusive { v >= l } else { v > l };
            let above = if inclusive { v <= u } else { v < u };
            return below && above;
        }
        let below = if inclusive { val >= lower } else { val > lower };
        let above = if inclusive { val <= upper } else { val < upper };
        below && above
    }
}
