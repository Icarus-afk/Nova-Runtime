#[derive(Debug, Clone)]
pub struct FacetResult {
    pub field: String,
    pub entries: Vec<(String, usize)>,
    pub total_docs: usize,
}

pub struct FacetCalculator;

impl FacetCalculator {
    pub fn calculate(
        entries: Vec<(String, String)>,
        facet_field: &str,
    ) -> FacetResult {
        let mut counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let total = entries.len();

        for (_doc_id, value) in entries.iter().filter(|(_, v)| !v.is_empty()) {
            *counts.entry(value.clone()).or_insert(0) += 1;
        }

        let mut entries: Vec<(String, usize)> = counts.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        FacetResult {
            field: facet_field.to_string(),
            entries,
            total_docs: total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facet_calculation() {
        let data = vec![
            ("1".to_string(), "fiction".to_string()),
            ("2".to_string(), "non-fiction".to_string()),
            ("3".to_string(), "fiction".to_string()),
            ("4".to_string(), "science".to_string()),
        ];
        let result = FacetCalculator::calculate(data, "category");
        assert_eq!(result.field, "category");
        assert_eq!(result.total_docs, 4);
        assert!(result.entries.iter().any(|(v, c)| v == "fiction" && *c == 2));
        assert!(result.entries.iter().any(|(v, c)| v == "non-fiction" && *c == 1));
        assert!(result.entries.iter().any(|(v, c)| v == "science" && *c == 1));
    }
}
