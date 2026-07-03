use super::ast::{BoolOperator, Query};
use crate::error::{Result, SearchError};

pub struct QueryParser;

impl QueryParser {
    pub fn parse(input: &str) -> Result<Query> {
        let input = input.trim();
        if input.is_empty() || input == "*:*" {
            return Ok(Query::MatchAll);
        }
        let tokens = Self::tokenize(input);
        let mut pos = 0;
        let result = Self::parse_or(&tokens, &mut pos)?;
        if pos < tokens.len() {
            return Err(SearchError::InvalidQuery(format!(
                "unexpected tokens at position {}: {:?}",
                pos, tokens[pos]
            )));
        }
        Ok(result)
    }

    fn tokenize(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();
        let mut current = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                '(' | ')' | ' ' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    if ch == '(' || ch == ')' {
                        tokens.push(ch.to_string());
                    }
                }
                '"' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    let mut phrase = String::new();
                    phrase.push('"');
                    while let Some(c) = chars.next() {
                        if c == '"' {
                            phrase.push('"');
                            break;
                        }
                        phrase.push(c);
                    }
                    tokens.push(phrase);
                }
                _ => {
                    current.push(ch);
                }
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        tokens
    }

    fn parse_or(tokens: &[String], pos: &mut usize) -> Result<Query> {
        let mut left = Self::parse_and(tokens, pos)?;

        while *pos < tokens.len() && tokens[*pos].to_uppercase() == "OR" {
            *pos += 1;
            let right = Self::parse_and(tokens, pos)?;
            left = Query::Bool {
                operator: BoolOperator::Or,
                clauses: vec![left, right],
            };
        }

        Ok(left)
    }

    fn parse_and(tokens: &[String], pos: &mut usize) -> Result<Query> {
        let mut left = Self::parse_not(tokens, pos)?;

        while *pos < tokens.len() {
            let tok = &tokens[*pos];
            if tok == ")" || tok.to_uppercase() == "OR" {
                break;
            }
            if tok.to_uppercase() == "AND" {
                *pos += 1;
            }
            let right = Self::parse_not(tokens, pos)?;
            left = Query::Bool {
                operator: BoolOperator::And,
                clauses: vec![left, right],
            };
        }

        Ok(left)
    }

    fn parse_not(tokens: &[String], pos: &mut usize) -> Result<Query> {
        if *pos >= tokens.len() {
            return Err(SearchError::InvalidQuery("unexpected end of query".into()));
        }

        let tok = &tokens[*pos];
        if tok == "-" || tok.to_uppercase() == "NOT" {
            *pos += 1;
            let clause = Self::parse_atom(tokens, pos)?;
            return Ok(Query::Bool {
                operator: BoolOperator::Not,
                clauses: vec![clause],
            });
        }

        if tok.starts_with('-') && tok.len() > 1 {
            let rest = &tok[1..];
            *pos += 1;
            let fake_tokens = vec![rest.to_string()];
            let mut fake_pos = 0;
            let clause = Self::parse_atom(&fake_tokens, &mut fake_pos)?;
            return Ok(Query::Bool {
                operator: BoolOperator::Not,
                clauses: vec![clause],
            });
        }

        Self::parse_atom(tokens, pos)
    }

    fn parse_atom(tokens: &[String], pos: &mut usize) -> Result<Query> {
        if *pos >= tokens.len() {
            return Err(SearchError::InvalidQuery("unexpected end of query".into()));
        }

        let tok = &tokens[*pos];

        if tok == "(" {
            *pos += 1;
            let query = Self::parse_or(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != ")" {
                return Err(SearchError::InvalidQuery("missing closing parenthesis".into()));
            }
            *pos += 1;
            return Ok(query);
        }

        if tok.starts_with('"') && tok.ends_with('"') && tok.len() >= 2 {
            *pos += 1;
            return Ok(Query::Phrase {
                field: None,
                value: tok[1..tok.len() - 1].to_string(),
                slop: 0,
            });
        }

        if tok == "*" || tok == "*:*" {
            *pos += 1;
            return Ok(Query::MatchAll);
        }

        let raw = tok.clone();
        *pos += 1;

        if let Some((field, rest)) = raw.split_once(':') {
            if rest.starts_with('[') || rest.starts_with('{') {
                return Self::parse_field_range(field.to_string(), rest, tokens, pos);
            }
                if rest.is_empty() && *pos < tokens.len() {
                    let next = &tokens[*pos];
                    if next.starts_with('"') && next.ends_with('"') && next.len() >= 2 {
                        *pos += 1;
                        return Ok(Query::Phrase {
                            field: Some(field.to_string()),
                            value: next[1..next.len() - 1].to_string(),
                            slop: 0,
                        });
                    }
                }
            return Self::parse_field_value(field.to_string(), rest);
        }

        Self::parse_term_value(&raw)
    }

    fn parse_field_value(field: String, value: &str) -> Result<Query> {
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            return Ok(Query::Phrase {
                field: Some(field),
                value: value[1..value.len() - 1].to_string(),
                slop: 0,
            });
        }

        if let Some(rest) = value.strip_suffix('*') {
            return Ok(Query::Prefix {
                field: Some(field),
                value: rest.to_string(),
            });
        }

        if let Some(rest) = value.strip_suffix('~') {
            return Ok(Query::Fuzzy {
                field: Some(field),
                value: rest.to_string(),
                max_distance: 2,
            });
        }

        if value.contains("~") {
            let parts: Vec<&str> = value.splitn(2, '~').collect();
            let dist = parts[1].parse::<u8>().unwrap_or(2);
            return Ok(Query::Fuzzy {
                field: Some(field),
                value: parts[0].to_string(),
                max_distance: dist,
            });
        }

        Ok(Query::Term {
            field: Some(field),
            value: value.to_string(),
        })
    }

    fn parse_field_range(field: String, value_start: &str, tokens: &[String], pos: &mut usize) -> Result<Query> {
        let inclusive = value_start.starts_with('[');
        let mut lower = value_start[1..].to_string();

        while *pos < tokens.len() && tokens[*pos].to_uppercase() != "TO" {
            lower.push(' ');
            lower.push_str(&tokens[*pos]);
            *pos += 1;
        }
        if *pos >= tokens.len() {
            return Err(SearchError::InvalidQuery("expected TO in range".into()));
        }
        *pos += 1;

        let mut upper = String::new();
        while *pos < tokens.len() && !tokens[*pos].ends_with(']') && !tokens[*pos].ends_with('}') {
            if !upper.is_empty() {
                upper.push(' ');
            }
            upper.push_str(&tokens[*pos]);
            *pos += 1;
        }
        if *pos >= tokens.len() {
            return Err(SearchError::InvalidQuery("expected ] or } in range".into()));
        }
        let closing = &tokens[*pos];
        if inclusive && !closing.ends_with(']') {
            return Err(SearchError::InvalidQuery("expected ] in range".into()));
        }
        if !inclusive && !closing.ends_with('}') {
            return Err(SearchError::InvalidQuery("expected } in range".into()));
        }
        let upper_val = closing.trim_end_matches(|c| c == ']' || c == '}');
        if !upper.is_empty() {
            upper.push(' ');
        }
        upper.push_str(upper_val);
        *pos += 1;

        Ok(Query::Range {
            field,
            lower: lower.trim().to_string(),
            upper: upper.trim().to_string(),
            inclusive,
        })
    }

    fn parse_term_value(raw: &str) -> Result<Query> {
        if let Some(rest) = raw.strip_suffix('*') {
            return Ok(Query::Prefix {
                field: None,
                value: rest.to_string(),
            });
        }

        if let Some(rest) = raw.strip_suffix('~') {
            return Ok(Query::Fuzzy {
                field: None,
                value: rest.to_string(),
                max_distance: 2,
            });
        }

        if raw.contains('~') {
            let parts: Vec<&str> = raw.splitn(2, '~').collect();
            let dist = parts[1].parse::<u8>().unwrap_or(2);
            return Ok(Query::Fuzzy {
                field: None,
                value: parts[0].to_string(),
                max_distance: dist,
            });
        }

        Ok(Query::Term {
            field: None,
            value: raw.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_term() {
        let q = QueryParser::parse("hello").unwrap();
        assert!(matches!(q, Query::Term { field: None, .. }));
    }

    #[test]
    fn test_parse_phrase() {
        let q = QueryParser::parse("\"hello world\"").unwrap();
        assert!(matches!(q, Query::Phrase { .. }));
    }

    #[test]
    fn test_parse_field_term() {
        let q = QueryParser::parse("title:hello").unwrap();
        match q {
            Query::Term { field, value } => {
                assert_eq!(field, Some("title".into()));
                assert_eq!(value, "hello");
            }
            _ => panic!("expected Term"),
        }
    }

    #[test]
    fn test_parse_prefix() {
        let q = QueryParser::parse("hello*").unwrap();
        assert!(matches!(q, Query::Prefix { .. }));
    }

    #[test]
    fn test_parse_fuzzy() {
        let q = QueryParser::parse("hello~").unwrap();
        assert!(matches!(q, Query::Fuzzy { .. }));
    }

    #[test]
    fn test_parse_fuzzy_distance() {
        let q = QueryParser::parse("hello~1").unwrap();
        match q {
            Query::Fuzzy { max_distance, .. } => assert_eq!(max_distance, 1),
            _ => panic!("expected Fuzzy"),
        }
    }

    #[test]
    fn test_parse_and() {
        let q = QueryParser::parse("hello world").unwrap();
        match q {
            Query::Bool { operator: BoolOperator::And, .. } => {}
            _ => panic!("expected Bool And"),
        }
    }

    #[test]
    fn test_parse_or() {
        let q = QueryParser::parse("hello OR world").unwrap();
        match q {
            Query::Bool { operator: BoolOperator::Or, .. } => {}
            _ => panic!("expected Bool Or"),
        }
    }

    #[test]
    fn test_parse_not() {
        let q = QueryParser::parse("-hello").unwrap();
        match q {
            Query::Bool { operator: BoolOperator::Not, .. } => {}
            _ => panic!("expected Bool Not"),
        }
    }

    #[test]
    fn test_parse_parens() {
        let q = QueryParser::parse("(hello world)").unwrap();
        match q {
            Query::Bool { operator: BoolOperator::And, .. } => {}
            _ => panic!("expected Bool And"),
        }
    }

    #[test]
    fn test_parse_range() {
        let q = QueryParser::parse("count:[1 TO 10]").unwrap();
        match q {
            Query::Range { field, lower, upper, inclusive } => {
                assert_eq!(field, "count");
                assert_eq!(lower, "1");
                assert_eq!(upper, "10");
                assert!(inclusive);
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn test_parse_empty() {
        let q = QueryParser::parse("").unwrap();
        assert!(matches!(q, Query::MatchAll));
    }

    #[test]
    fn test_parse_wildcard() {
        let q = QueryParser::parse("*").unwrap();
        assert!(matches!(q, Query::MatchAll));
    }
}
