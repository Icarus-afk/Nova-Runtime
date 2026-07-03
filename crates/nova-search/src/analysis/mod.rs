pub mod tokenizer;
pub mod stemmer;
pub mod stop_words;

use tokenizer::{Token, TokenizerKind};
use stemmer::PorterStemmer;
use stop_words::StopWordsFilter;

pub struct AnalyzerPipeline {
    tokenizer_kind: TokenizerKind,
    #[allow(dead_code)]
    stemmer: PorterStemmer,
    stop_words: StopWordsFilter,
}

impl Default for AnalyzerPipeline {
    fn default() -> Self {
        AnalyzerPipeline {
            tokenizer_kind: TokenizerKind::Standard,
            stemmer: PorterStemmer,
            stop_words: StopWordsFilter::default(),
        }
    }
}

impl AnalyzerPipeline {
    pub fn new(tokenizer_kind: TokenizerKind, stop_words: StopWordsFilter) -> Self {
        AnalyzerPipeline {
            tokenizer_kind,
            stemmer: PorterStemmer,
            stop_words,
        }
    }

    pub fn analyze(&self, text: &str) -> Vec<Token> {
        let tokens = tokenizer::tokenize(text, self.tokenizer_kind);
        let tokens = self.stop_words.filter(tokens);
        tokens
            .into_iter()
            .map(|t| {
                let stemmed = PorterStemmer::stem(&t.term);
                Token {
                    term: stemmed,
                    ..t
                }
            })
            .collect()
    }

    pub fn analyze_query(&self, text: &str) -> Vec<Token> {
        let tokens = tokenizer::tokenize(text, self.tokenizer_kind);
        tokens
            .into_iter()
            .map(|t| {
                let stemmed = PorterStemmer::stem(&t.term);
                Token {
                    term: stemmed,
                    ..t
                }
            })
            .collect()
    }
}
