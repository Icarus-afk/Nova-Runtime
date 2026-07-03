use crate::analysis::tokenizer::Token;

fn char_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let mut i = index;
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub struct Highlighter;

impl Highlighter {
    pub fn highlight(text: &str, tokens: &[Token], snippet_len: usize) -> Vec<String> {
        if tokens.is_empty() {
            let end = char_boundary(text, std::cmp::min(snippet_len, text.len()));
            return vec![text[..end].to_string()];
        }

        let lower_text = text.to_lowercase();
        let mut match_positions: Vec<(usize, usize)> = Vec::new();

        for token in tokens {
            let term_lower = token.term.to_lowercase();
            let mut search_start = 0;
            while let Some(pos) = lower_text[search_start..].find(&term_lower) {
                let abs_pos = search_start + pos;
                let end = abs_pos + term_lower.len();
                if end <= text.len() {
                    match_positions.push((abs_pos, end));
                }
                search_start = abs_pos.saturating_add(term_lower.len());
                if search_start > lower_text.len() {
                    break;
                }
            }
        }

        match_positions.sort();
        match_positions.dedup();

        if match_positions.is_empty() {
            let end = char_boundary(text, std::cmp::min(snippet_len, text.len()));
            return vec![text[..end].to_string()];
        }

        let mut snippets: Vec<String> = Vec::new();
        let mut covered: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for &(start, end) in &match_positions {
            if covered.iter().any(|&c| c >= start && c < end) {
                continue;
            }

            let raw_snippet_start = if start > snippet_len / 2 {
                start - snippet_len / 2
            } else {
                0
            };

            let snippet_start = char_boundary(text, raw_snippet_start);
            let raw_snippet_end = std::cmp::min(snippet_start + snippet_len, text.len());
            let snippet_end = char_boundary(text, raw_snippet_end);
            let mut snippet = text[snippet_start..snippet_end].to_string();

            for (ms, me) in &match_positions {
                if *ms >= snippet_start && *me <= snippet_end {
                    let adjusted_start = *ms - snippet_start;
                    let adjusted_end = *me - snippet_start;
                    let term = &snippet[adjusted_start..adjusted_end];
                    let highlighted = format!("\x1b[1m{}\x1b[0m", term);
                    snippet.replace_range(adjusted_start..adjusted_end, &highlighted);
                    covered.insert(*ms);
                }
            }

            if snippet_start > 0 {
                snippet = format!("...{}", snippet);
            }
            if snippet_end < text.len() {
                snippet = format!("{}...", snippet);
            }

            snippets.push(snippet);
        }

        if snippets.is_empty() {
            let end = char_boundary(text, std::cmp::min(snippet_len, text.len()));
            snippets.push(text[..end].to_string());
        }

        snippets.truncate(3);
        snippets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::tokenizer::Token;

    #[test]
    fn test_highlight_basic() {
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = vec![Token {
            term: "fox".to_string(),
            start_offset: 0,
            end_offset: 3,
            position: 0,
        }];
        let snippets = Highlighter::highlight(text, &tokens, 150);
        assert!(!snippets.is_empty());
        assert!(snippets[0].contains("fox"));
    }

    #[test]
    fn test_highlight_empty_tokens() {
        let text = "hello world";
        let snippets = Highlighter::highlight(text, &[], 150);
        assert!(!snippets.is_empty());
    }

    #[test]
    fn test_unicode_highlighting() {
        let text = "Café au lait and résumé data";
        let tokens = vec![Token {
            term: "café".to_string(),
            start_offset: 0,
            end_offset: 4,
            position: 0,
        }];
        let snippets = Highlighter::highlight(text, &tokens, 150);
        assert!(!snippets.is_empty());
        assert!(snippets[0].contains("Café"));

        let emoji_text = "Hello 🌍 world 🌎 test";
        let emoji_tokens = vec![Token {
            term: "🌍".to_string(),
            start_offset: 6,
            end_offset: 10,
            position: 0,
        }];
        let emoji_snippets = Highlighter::highlight(emoji_text, &emoji_tokens, 150);
        assert!(!emoji_snippets.is_empty());

        let cjk = "日本語テスト文章";
        let cjk_end = std::cmp::min(3, cjk.len());
        let cjk_snippets = Highlighter::highlight(cjk, &[], 3);
        assert!(!cjk_snippets.is_empty());
    }
}
