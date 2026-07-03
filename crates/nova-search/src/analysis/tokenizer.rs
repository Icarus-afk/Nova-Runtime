use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Token {
    pub term: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub position: usize,
}

pub struct StandardTokenizer;

impl StandardTokenizer {
    pub fn tokenize(text: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut position = 0;

        for word in text.split_word_bounds() {
            if word.trim().is_empty() {
                continue;
            }

            let is_punctuation = word.chars().all(|c| c.is_ascii_punctuation() || c.is_whitespace());

            if is_punctuation && word.len() == 1 {
                continue;
            }

            let clean = word
                .trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
                .to_lowercase();

            if clean.is_empty() {
                continue;
            }

            let start_offset = word.as_ptr() as usize - text.as_ptr() as usize;
            let end_offset = start_offset + word.len();

            tokens.push(Token {
                term: clean,
                start_offset,
                end_offset,
                position,
            });
            position += 1;
        }

        tokens
    }
}

pub struct WhitespaceTokenizer;

impl WhitespaceTokenizer {
    pub fn tokenize(text: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut position = 0;
        let mut pos = 0;

        for word in text.split_whitespace() {
            let term = word.to_lowercase();
            let start = pos + word.as_ptr() as usize - text[pos..].as_ptr() as usize;
            let end = start + word.len();
            pos = end;

            tokens.push(Token {
                term,
                start_offset: start,
                end_offset: end,
                position,
            });
            position += 1;
        }

        tokens
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TokenizerKind {
    Standard,
    Whitespace,
}

pub fn tokenize(text: &str, kind: TokenizerKind) -> Vec<Token> {
    match kind {
        TokenizerKind::Standard => StandardTokenizer::tokenize(text),
        TokenizerKind::Whitespace => WhitespaceTokenizer::tokenize(text),
    }
}
