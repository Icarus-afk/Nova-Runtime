#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select, From, Where, Insert, Into, Values, Update, Set, Delete, Create, Table, Drop,
    And, Or, Not, Null, Is, In, Between, Like, True, False, As, On, Group, By, Having,
    Order, Asc, Desc,     Limit, Offset, Distinct, All, Exists, Default,
    Count, Sum, Avg, Min, Max,
    // Identifiers & literals
    Identifier(String),
    Number(String),
    String(String),
    // Operators
    Plus, Minus, Star, Slash, Percent,
    Eq, NotEq, Lt, LtEq, Gt, GtEq, Concat,
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

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            if self.pos >= self.chars.len() {
                tokens.push(Token::EOF);
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
                self.read_operator_or_punct()
            };
            tokens.push(token);
        }
        tokens
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
        self.pos += 1; // skip opening '
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
            _ => Token::Identifier(word),
        }
    }

    fn read_operator_or_punct(&mut self) -> Token {
        let ch = self.chars[self.pos];
        self.pos += 1;
        match ch {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Percent,
            '=' => Token::Eq,
            '!' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Token::NotEq
                } else {
                    Token::Not
                }
            }
            '<' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Token::LtEq
                } else if self.pos < self.chars.len() && self.chars[self.pos] == '>' {
                    self.pos += 1;
                    Token::NotEq
                } else {
                    Token::Lt
                }
            }
            '>' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                    self.pos += 1;
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }
            '(' => Token::LParen,
            ')' => Token::RParen,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            '.' => Token::Dot,
            '|' => {
                if self.pos < self.chars.len() && self.chars[self.pos] == '|' {
                    self.pos += 1;
                    Token::Concat
                } else {
                    panic!("unexpected character: |")
                }
            }
            _ => panic!("unexpected character: {}", ch),
        }
    }
}
