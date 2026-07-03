#[derive(Debug, Clone)]
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    CreateTable(CreateTableStatement),
    DropTable(DropTableStatement),
}

#[derive(Debug, Clone)]
pub struct SelectStatement {
    pub select_list: Vec<SelectItem>,
    pub from: TableRef,
    pub where_clause: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByExpr>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SelectItem {
    Expr { expr: Expr, alias: Option<String> },
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InsertStatement {
    pub table: TableRef,
    pub columns: Vec<String>,
    pub values: Vec<Vec<Expr>>,
}

#[derive(Debug, Clone)]
pub struct UpdateStatement {
    pub table: TableRef,
    pub assignments: Vec<Assignment>,
    pub where_clause: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub column: String,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct DeleteStatement {
    pub table: TableRef,
    pub where_clause: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct CreateTableStatement {
    pub table: TableRef,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone)]
pub struct DropTableStatement {
    pub table: TableRef,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Column(String),
    Literal(LiteralValue),
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expr>,
    },
    Function {
        name: String,
        args: Vec<Expr>,
    },
    IsNull(Box<Expr>),
    IsNotNull(Box<Expr>),
    In {
        expr: Box<Expr>,
        list: Vec<Expr>,
    },
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
    },
    Like {
        expr: Box<Expr>,
        pattern: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Concat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SQLType {
    Null,
    Boolean,
    Integer,
    Float,
    Text,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub sql_type: SQLType,
    pub nullable: bool,
    pub default: Option<LiteralValue>,
}

#[derive(Debug, Clone)]
pub struct OrderByExpr {
    pub expr: Expr,
    pub asc: bool,
}
