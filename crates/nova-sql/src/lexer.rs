use crate::error::{Result, SQLError};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select, From, Where, Insert, Into, Values, Update, Set, Delete, Create, Table, Drop,
    And, Or, Not, Null, Is, In, Between, Like, ILike, True, False, As, On, Group, By, Having,
    Order, Asc, Desc, Limit, Offset, Distinct, All, Exists, Default,
    Count, Sum, Avg, Min, Max, Case, When, Then, Else, End, Cast,
    Primary, Key, Unique, Nulls, First, Last, Check,
    // Identifiers & literals
    Identifier(String),
    Number(String),
    String(String),
    // Operators
    Plus, Minus, Star, Slash, Percent,
    Eq, NotEq, Lt, LtEq, Gt, GtEq, Concat, ColonColon,
    // Punctuation
    LParen, RParen, Comma, Semicolon, Dot,
    // Special
    EOF,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<(Vec<Token>, Vec<(usize, usize)>)> {
        let mut tokens = Vec::new();
        let mut positions = Vec::new();
        loop {
            self.skip_whitespace();
            let start = self.pos;
            if self.pos >= self.chars.len() {
                tokens.push(Token::EOF);
                positions.push((start, start));
                break;
            }
            let ch = self.chars[self.pos];
            if ch == '-' && self.peek() == Some('-') {
                self.skip_line_comment();
                continue;
            }
            let token = if ch == '\'' {
                self.read_string()
            } else if ch.is_ascii_digit() {
                self.read_number()
            } else if ch.is_ascii_alphabetic() || ch == '_' {
                self.read_identifier_or_keyword()
            } else {
                self.read_operator_or_punct()?
            };
            let end = self.pos;
            tokens.push(token);
            positions.push((start, end));
        }
        Ok((tokens, positions))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn skip_line_comment(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos] != '\n' {
            self.pos += 1;
        }
    }

    fn read_string(&mut self) -> Token {
        self.pos += 1;
        let mut s = String::new();
        loop {
            if self.pos >= self.chars.len() {
                break;
            }
            let ch = self.chars[self.pos];
            if ch == '\'' {
                if self.peek() == Some('\'') {
                    s.push('\'');
                    self.pos += 2;
                } else {
                    self.pos += 1;
                    break;
                }
            } else {
                s.push(ch);
                self.pos += 1;
            }
        }
        Token::String(s)
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.chars.len() && self.chars[self.pos] == '.' {
            let next = self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false);
            if next {
                self.pos += 1;
                while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            }
        }
        if self.pos < self.chars.len()
            && (self.chars[self.pos] == 'e' || self.chars[self.pos] == 'E')
        {
            self.pos += 1;
            if self.pos < self.chars.len()
                && (self.chars[self.pos] == '+' || self.chars[self.pos] == '-')
            {
                self.pos += 1;
            }
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        Token::Number(self.chars[start..self.pos].iter().collect())
    }

    fn read_identifier_or_keyword(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        let word: String = self.chars[start..self.pos].iter().collect();
        let lower = word.to_lowercase();
        match lower.as_str() {
            "select" => Token::Select,
            "from" => Token::From,
            "where" => Token::Where,
            "insert" => Token::Insert,
            "into" => Token::Into,
            "values" => Token::Values,
            "update" => Token::Update,
            "set" => Token::Set,
            "delete" => Token::Delete,
            "create" => Token::Create,
            "table" => Token::Table,
            "drop" => Token::Drop,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            "null" => Token::Null,
            "is" => Token::Is,
            "in" => Token::In,
            "between" => Token::Between,
            "like" => Token::Like,
            "ilike" => Token::ILike,
            "true" => Token::True,
            "false" => Token::False,
            "as" => Token::As,
            "on" => Token::On,
            "group" => Token::Group,
            "by" => Token::By,
            "having" => Token::Having,
            "order" => Token::Order,
            "asc" => Token::Asc,
            "desc" => Token::Desc,
            "limit" => Token::Limit,
            "offset" => Token::Offset,
            "distinct" => Token::Distinct,
            "all" => Token::All,
            "exists" => Token::Exists,
            "default" => Token::Default,
            "count" => Token::Count,
            "sum" => Token::Sum,
            "avg" => Token::Avg,
            "min" => Token::Min,
            "max" => Token::Max,
            "case" => Token::Case,
            "when" => Token::When,
            "then" => Token::Then,
            "else" => Token::Else,
            "end" => Token::End,
            "cast" => Token::Cast,
            "primary" => Token::Primary,
            "key" => Token::Key,
            "unique" => Token::Unique,
            "check" => Token::Check,
            "nulls" => Token::Nulls,
            "first" => Token::First,
            "last" => Token::Last,
            _ => Token::Identifier(word),
        }
    }

    fn read_operator_or_punct(&mut self) -> Result<Token> {
        let ch = self.chars[self.pos];
        self.pos += 1;
        match ch {
            '+' => Ok(Token::Plus),
            '-' => Ok(Token::Minus),
            '*' => Ok(Token::Star),
            '/' => Ok(Token::Slash),
            '%' => Ok(Token::Percent),
            '=' => Ok(Token::Eq),
            '!' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Ok(Token::NotEq)
                } else {
                    Ok(Token::Not)
                }
            }
            '<' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Ok(Token::LtEq)
                } else if self.pos < self.chars.len() && self.chars[self.pos] == '>' {
                    self.pos += 1;
                    Ok(Token::NotEq)
                } else {
                    Ok(Token::Lt)
                }
            }
            '>' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Ok(Token::GtEq)
                } else {
                    Ok(Token::Gt)
                }
            }
            '(' => Ok(Token::LParen),
            ')' => Ok(Token::RParen),
            ',' => Ok(Token::Comma),
            ';' => Ok(Token::Semicolon),
            '.' => Ok(Token::Dot),
            ':' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == ':' {
                    self.pos += 1;
                    Ok(Token::ColonColon)
                } else {
                    Err(SQLError::syntax_at("unexpected character: ':'", self.pos - 1, self.pos))
                }
            }
            '|' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '|' {
                    self.pos += 1;
                    Ok(Token::Concat)
                } else {
                    Err(SQLError::syntax_at(
                        format!("unexpected character: |"),
                        self.pos - 1,
                        self.pos,
                    ))
                }
            }
            c => Err(SQLError::syntax_at(
                format!("unexpected character: {}", c),
                self.pos - 1,
                self.pos,
            )),
        }
    }
}
