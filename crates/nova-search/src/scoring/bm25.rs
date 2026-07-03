use crate::index::segment::InMemorySegment;

pub struct BM25Scorer {
    k1: f64,
    b: f64,
    avg_field_lengths: std::collections::HashMap<String, f64>,
    total_docs: u64,
}

impl BM25Scorer {
    pub fn new(segment: &InMemorySegment) -> Self {
        let mut avg_field_lengths = std::collections::HashMap::new();
        for (field, lengths) in &segment.field_lengths {
            let total: u64 = lengths.values().sum();
            let count = lengths.len() as u64;
            let avg = if count > 0 { total as f64 / count as f64 } else { 0.0 };
            avg_field_lengths.insert(field.clone(), avg);
        }

        BM25Scorer {
            k1: 1.2,
            b: 0.75,
            avg_field_lengths,
            total_docs: segment.doc_count,
        }
    }

    pub fn with_params(k1: f64, b: f64) -> Self {
        BM25Scorer {
            k1,
            b,
            avg_field_lengths: std::collections::HashMap::new(),
            total_docs: 0,
        }
    }

    pub fn idf(total_docs: u64, doc_freq: u64) -> f64 {
        if doc_freq == 0 {
            return 0.0;
        }
        let numerator = (total_docs as f64 - doc_freq as f64 + 0.5) / (doc_freq as f64 + 0.5);
        (numerator + 1.0).ln()
    }

    pub fn tf_score(&self, term_freq: f64, field_length: f64, avg_field_length: f64) -> f64 {
        if avg_field_length <= 0.0 {
            return term_freq / (term_freq + self.k1);
        }
        let numerator = term_freq * (self.k1 + 1.0);
        let denominator = term_freq + self.k1 * (1.0 - self.b + self.b * field_length / avg_field_length);
        numerator / denominator
    }

    pub fn score(&self, term_freq: f64, doc_freq: u64, field_length: f64, avg_field_length: f64) -> f64 {
        let idf = Self::idf(self.total_docs, doc_freq);
        let tf = self.tf_score(term_freq, field_length, avg_field_length);
        idf * tf
    }

    pub fn avg_field_length(&self, field: &str) -> f64 {
        self.avg_field_lengths.get(field).copied().unwrap_or(0.0)
    }
}
