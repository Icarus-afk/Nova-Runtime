pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr_row[j + 1] = std::cmp::min(
                std::cmp::min(curr_row[j] + 1, prev_row[j + 1] + 1),
                prev_row[j] + cost,
            );
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

pub fn find_fuzzy_matches(term: &str, candidates: &[String], max_distance: u8) -> Vec<String> {
    let max_dist = max_distance as usize;
    candidates
        .iter()
        .filter(|c| {
            let dist = levenshtein_distance(term, c);
            dist <= max_dist
        })
        .cloned()
        .collect()
}

#[derive(Debug, Clone)]
pub struct LevenshteinAutomaton {
    pattern: String,
    max_distance: u8,
}

impl LevenshteinAutomaton {
    pub fn new(pattern: &str, max_distance: u8) -> Self {
        LevenshteinAutomaton {
            pattern: pattern.to_string(),
            max_distance,
        }
    }

    pub fn matches(&self, text: &str) -> bool {
        levenshtein_distance(&self.pattern, text) <= self.max_distance as usize
    }

    pub fn find_matches<'a>(&self, candidates: &'a [String]) -> Vec<&'a str> {
        candidates
            .iter()
            .filter(|c| self.matches(c))
            .map(|s| s.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance_zero() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_distance_insertion() {
        assert_eq!(levenshtein_distance("cat", "cats"), 1);
    }

    #[test]
    fn test_levenshtein_distance_deletion() {
        assert_eq!(levenshtein_distance("cats", "cat"), 1);
    }

    #[test]
    fn test_levenshtein_distance_substitution() {
        assert_eq!(levenshtein_distance("cat", "car"), 1);
    }

    #[test]
    fn test_levenshtein_distance_two_edits() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_levenshtein_distance_empty() {
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn test_find_fuzzy_matches() {
        let candidates = vec![
            "hello".to_string(),
            "hallo".to_string(),
            "hullo".to_string(),
            "world".to_string(),
            "helo".to_string(),
        ];
        let matches = find_fuzzy_matches("hello", &candidates, 1);
        assert!(matches.contains(&"hello".to_string()));
        assert!(matches.contains(&"hallo".to_string()));
        assert!(!matches.contains(&"world".to_string()));
    }

    #[test]
    fn test_automaton() {
        let auto = LevenshteinAutomaton::new("hello", 1);
        assert!(auto.matches("hello"));
        assert!(auto.matches("hallo"));
        assert!(!auto.matches("world"));
    }
}
