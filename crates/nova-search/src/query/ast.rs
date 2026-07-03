#[derive(Debug, Clone)]
pub enum Query {
    Term {
        field: Option<String>,
        value: String,
    },
    Phrase {
        field: Option<String>,
        value: String,
        slop: u32,
    },
    Prefix {
        field: Option<String>,
        value: String,
    },
    Fuzzy {
        field: Option<String>,
        value: String,
        max_distance: u8,
    },
    Range {
        field: String,
        lower: String,
        upper: String,
        inclusive: bool,
    },
    Bool {
        operator: BoolOperator,
        clauses: Vec<Query>,
    },
    MatchAll,
}

#[derive(Debug, Clone, Copy)]
pub enum BoolOperator {
    And,
    Or,
    Not,
}

impl Query {
    pub fn term(field: Option<impl Into<String>>, value: impl Into<String>) -> Self {
        Query::Term {
            field: field.map(|f| f.into()),
            value: value.into(),
        }
    }

    pub fn phrase(field: Option<impl Into<String>>, value: impl Into<String>) -> Self {
        Query::Phrase {
            field: field.map(|f| f.into()),
            value: value.into(),
            slop: 0,
        }
    }

    pub fn prefix(field: Option<impl Into<String>>, value: impl Into<String>) -> Self {
        Query::Prefix {
            field: field.map(|f| f.into()),
            value: value.into(),
        }
    }

    pub fn fuzzy(field: Option<impl Into<String>>, value: impl Into<String>, max_distance: u8) -> Self {
        Query::Fuzzy {
            field: field.map(|f| f.into()),
            value: value.into(),
            max_distance,
        }
    }

    pub fn match_all() -> Self {
        Query::MatchAll
    }
}
