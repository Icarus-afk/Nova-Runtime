use crate::ast::*;
use crate::error::{Result, SQLError};
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();
        while self.peek() != Token::EOF {
            statements.push(self.parse_statement()?);
            if self.peek() == Token::Semicolon {
                self.advance();
            }
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        match self.peek() {
            Token::Select => {
                let sel = self.parse_select()?;
                Ok(Statement::Select(sel))
            }
            Token::Insert => {
                let ins = self.parse_insert()?;
                Ok(Statement::Insert(ins))
            }
            Token::Update => {
                let upd = self.parse_update()?;
                Ok(Statement::Update(upd))
            }
            Token::Delete => {
                let del = self.parse_delete()?;
                Ok(Statement::Delete(del))
            }
            Token::Create => {
                let ct = self.parse_create_table()?;
                Ok(Statement::CreateTable(ct))
            }
            Token::Drop => {
                let dt = self.parse_drop_table()?;
                Ok(Statement::DropTable(dt))
            }
            t => Err(SQLError::Syntax(format!(
                "unexpected token at start of statement: {:?}",
                t
            ))),
        }
    }

    // SELECT select_list FROM table_ref [WHERE expr]
    //   [GROUP BY expr [, ...]] [HAVING expr]
    //   [ORDER BY expr [ASC|DESC] [, ...]]
    //   [LIMIT n] [OFFSET n]
    fn parse_select(&mut self) -> Result<SelectStatement> {
        self.expect(Token::Select)?;
        let select_list = self.parse_select_list()?;
        self.expect(Token::From)?;
        let from = self.parse_table_ref()?;
        let where_clause = if self.eat_if(Token::Where) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let group_by = if self.eat_if(Token::Group) {
            self.expect(Token::By)?;
            self.parse_comma_separated(Self::parse_expression)?
        } else {
            Vec::new()
        };
        let having = if self.eat_if(Token::Having) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let order_by = if self.eat_if(Token::Order) {
            self.expect(Token::By)?;
            self.parse_comma_separated(Self::parse_order_by_expr)?
        } else {
            Vec::new()
        };
        let limit = if self.eat_if(Token::Limit) {
            Some(self.parse_usize()?)
        } else {
            None
        };
        let offset = if self.eat_if(Token::Offset) {
            Some(self.parse_usize()?)
        } else {
            None
        };
        Ok(SelectStatement {
            select_list,
            from,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
        })
    }

    fn parse_select_list(&mut self) -> Result<Vec<SelectItem>> {
        self.parse_comma_separated(Self::parse_select_item)
    }

    fn parse_select_item(&mut self) -> Result<SelectItem> {
        if self.eat_if(Token::Star) {
            return Ok(SelectItem::Wildcard);
        }
        let expr = self.parse_expression()?;
        let alias = if self.eat_if(Token::As) {
            Some(self.parse_identifier()?)
        } else if self.peek_is_identifier() {
            let saved = self.pos;
            let ident = self.parse_identifier()?;
            // Check if the next token suggests this is an alias vs. part of a larger expression
            match self.peek() {
                Token::Comma | Token::From | Token::Where
                | Token::Group | Token::Order | Token::Having
                | Token::Limit | Token::Offset | Token::Semicolon | Token::EOF => {
                    Some(ident)
                }
                _ => {
                    self.pos = saved;
                    None
                }
            }
        } else {
            None
        };
        Ok(SelectItem::Expr { expr, alias })
    }

    fn parse_table_ref(&mut self) -> Result<TableRef> {
        let name = self.parse_identifier()?;
        let alias = if self.eat_if(Token::As) {
            Some(self.parse_identifier()?)
        } else if self.peek_is_identifier() {
            let saved = self.pos;
            let ident = self.parse_identifier()?;
            match self.peek() {
                Token::Where | Token::Group | Token::Order
                | Token::Having | Token::Limit | Token::Offset
                | Token::Comma | Token::Semicolon | Token::EOF => Some(ident),
                _ => {
                    self.pos = saved;
                    None
                }
            }
        } else {
            None
        };
        Ok(TableRef { name, alias })
    }

    fn parse_order_by_expr(&mut self) -> Result<OrderByExpr> {
        let expr = self.parse_expression()?;
        let asc = if self.eat_if(Token::Asc) {
            true
        } else if self.eat_if(Token::Desc) {
            false
        } else {
            true
        };
        Ok(OrderByExpr { expr, asc })
    }

    // INSERT INTO table_ref [(col, ...)] VALUES (val, ...) [, (val, ...)]
    fn parse_insert(&mut self) -> Result<InsertStatement> {
        self.expect(Token::Insert)?;
        self.expect(Token::Into)?;
        let table = self.parse_table_ref()?;
        let columns = if self.eat_if(Token::LParen) {
            let cols = self.parse_comma_separated(Self::parse_identifier)?;
            self.expect(Token::RParen)?;
            cols
        } else {
            Vec::new()
        };
        self.expect(Token::Values)?;
        let values = self.parse_comma_separated(Self::parse_value_list)?;
        Ok(InsertStatement {
            table,
            columns,
            values,
        })
    }

    fn parse_value_list(&mut self) -> Result<Vec<Expr>> {
        self.expect(Token::LParen)?;
        let exprs = self.parse_comma_separated(Self::parse_expression)?;
        self.expect(Token::RParen)?;
        Ok(exprs)
    }

    // UPDATE table SET col = expr [, col = expr] [WHERE expr]
    fn parse_update(&mut self) -> Result<UpdateStatement> {
        self.expect(Token::Update)?;
        let table = self.parse_table_ref()?;
        self.expect(Token::Set)?;
        let assignments = self.parse_comma_separated(Self::parse_assignment)?;
        let where_clause = if self.eat_if(Token::Where) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        Ok(UpdateStatement {
            table,
            assignments,
            where_clause,
        })
    }

    fn parse_assignment(&mut self) -> Result<Assignment> {
        let column = self.parse_identifier()?;
        self.expect(Token::Eq)?;
        let value = self.parse_expression()?;
        Ok(Assignment { column, value })
    }

    // DELETE FROM table [WHERE expr]
    fn parse_delete(&mut self) -> Result<DeleteStatement> {
        self.expect(Token::Delete)?;
        self.expect(Token::From)?;
        let table = self.parse_table_ref()?;
        let where_clause = if self.eat_if(Token::Where) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        Ok(DeleteStatement {
            table,
            where_clause,
        })
    }

    // CREATE TABLE table (col_def, ...)
    fn parse_create_table(&mut self) -> Result<CreateTableStatement> {
        self.expect(Token::Create)?;
        self.expect(Token::Table)?;
        let table = self.parse_table_ref()?;
        self.expect(Token::LParen)?;
        let columns = self.parse_comma_separated(Self::parse_column_def)?;
        self.expect(Token::RParen)?;
        Ok(CreateTableStatement { table, columns })
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef> {
        let name = self.parse_identifier()?;
        let sql_type = self.parse_sql_type()?;
        let nullable = if self.eat_if(Token::Not) {
            self.expect(Token::Null)?;
            false
        } else if self.eat_if(Token::Null) {
            true
        } else {
            true
        };
        let default = if self.eat_if(Token::Default) {
            Some(self.parse_literal()?)
        } else {
            None
        };
        Ok(ColumnDef {
            name,
            sql_type,
            nullable,
            default,
        })
    }

    fn parse_sql_type(&mut self) -> Result<SQLType> {
        let ident = self.parse_identifier()?;
        match ident.to_lowercase().as_str() {
            "int" | "integer" => Ok(SQLType::Integer),
            "float" | "double" | "real" => Ok(SQLType::Float),
            "text" | "varchar" | "string" => Ok(SQLType::Text),
            "bool" | "boolean" => Ok(SQLType::Boolean),
            other => Err(SQLError::Syntax(format!("unknown type: {}", other))),
        }
    }

    // DROP TABLE table
    fn parse_drop_table(&mut self) -> Result<DropTableStatement> {
        self.expect(Token::Drop)?;
        self.expect(Token::Table)?;
        let table = self.parse_table_ref()?;
        Ok(DropTableStatement { table })
    }

    // Expression parsing with precedence climbing
    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;
        while self.eat_if(Token::Or) {
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;
        while self.eat_if(Token::And) {
            let right = self.parse_equality()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            if self.eat_if(Token::Eq) {
                let right = self.parse_comparison()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Eq,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::NotEq) {
                let right = self.parse_comparison()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::NotEq,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_term()?;
        loop {
            if self.eat_if(Token::Lt) {
                let right = self.parse_term()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Lt,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::LtEq) {
                let right = self.parse_term()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::LtEq,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::Gt) {
                let right = self.parse_term()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Gt,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::GtEq) {
                let right = self.parse_term()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::GtEq,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            if self.eat_if(Token::Plus) {
                let right = self.parse_factor()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Plus,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::Minus) {
                let right = self.parse_factor()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Minus,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::Concat) {
                let right = self.parse_factor()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Concat,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            if self.eat_if(Token::Star) {
                let right = self.parse_unary()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Multiply,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::Slash) {
                let right = self.parse_unary()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Divide,
                    right: Box::new(right),
                };
            } else if self.eat_if(Token::Percent) {
                let right = self.parse_unary()?;
                left = Expr::BinaryOp {
                    left: Box::new(left),
                    op: BinaryOperator::Modulo,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        if self.eat_if(Token::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Neg,
                expr: Box::new(expr),
            });
        }
        if self.eat_if(Token::Not) {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Not,
                expr: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        if self.eat_if(Token::LParen) {
            let expr = self.parse_expression()?;
            self.expect(Token::RParen)?;
            return Ok(expr);
        }
        if let Some(token) = self.tokens.get(self.pos).cloned() {
            match token {
                Token::Count | Token::Sum | Token::Avg | Token::Min | Token::Max => {
                    return self.parse_aggregate_function();
                }
                Token::Exists => {
                    return self.parse_exists();
                }
                _ => {}
            }
        }
        if self.eat_if(Token::Null) {
            return Ok(Expr::Literal(LiteralValue::Null));
        }
        if self.eat_if(Token::True) {
            return Ok(Expr::Literal(LiteralValue::Boolean(true)));
        }
        if self.eat_if(Token::False) {
            return Ok(Expr::Literal(LiteralValue::Boolean(false)));
        }
        if let Token::Number(s) = self.peek() {
            self.advance();
            return self.make_number_literal(&s);
        }
        if let Token::String(s) = self.peek() {
            self.advance();
            return Ok(Expr::Literal(LiteralValue::String(s)));
        }
        if self.peek_is_identifier() {
            let name = self.parse_identifier()?;
            if self.eat_if(Token::LParen) {
                // Function call
                let args = self.parse_comma_separated(Self::parse_expression)?;
                self.expect(Token::RParen)?;
                return Ok(Expr::Function { name, args });
            }
            // Could be followed by IS NULL/IN/BETWEEN/LIKE
            let mut expr = Expr::Column(name);
            // Postfix operators
            if self.eat_if(Token::Is) {
                if self.eat_if(Token::Not) {
                    self.expect(Token::Null)?;
                    expr = Expr::IsNotNull(Box::new(expr));
                } else {
                    self.expect(Token::Null)?;
                    expr = Expr::IsNull(Box::new(expr));
                }
            } else if self.eat_if(Token::In) {
                self.expect(Token::LParen)?;
                let list = self.parse_comma_separated(Self::parse_expression)?;
                self.expect(Token::RParen)?;
                expr = Expr::In {
                    expr: Box::new(expr),
                    list,
                };
            } else if self.eat_if(Token::Between) {
                let low = self.parse_expression()?;
                self.expect(Token::And)?;
                let high = self.parse_expression()?;
                expr = Expr::Between {
                    expr: Box::new(expr),
                    low: Box::new(low),
                    high: Box::new(high),
                };
            } else if self.eat_if(Token::Like) {
                let pattern = self.parse_expression()?;
                expr = Expr::Like {
                    expr: Box::new(expr),
                    pattern: Box::new(pattern),
                };
            }
            return Ok(expr);
        }
        Err(SQLError::Syntax(format!(
            "unexpected token: {:?}",
            self.peek()
        )))
    }

    fn parse_aggregate_function(&mut self) -> Result<Expr> {
        let name = match self.peek() {
            Token::Count => {
                self.advance();
                "count".to_string()
            }
            Token::Sum => {
                self.advance();
                "sum".to_string()
            }
            Token::Avg => {
                self.advance();
                "avg".to_string()
            }
            Token::Min => {
                self.advance();
                "min".to_string()
            }
            Token::Max => {
                self.advance();
                "max".to_string()
            }
            _ => unreachable!(),
        };
        self.expect(Token::LParen)?;
        let args = if self.eat_if(Token::Star) {
            vec![Expr::Literal(LiteralValue::String("*".to_string()))]
        } else if self.eat_if(Token::Distinct) {
            let arg = self.parse_expression()?;
            self.expect(Token::RParen)?;
            return Ok(Expr::Function {
                name: format!("{}_distinct", name),
                args: vec![arg],
            });
        } else {
            self.parse_comma_separated(Self::parse_expression)?
        };
        self.expect(Token::RParen)?;
        Ok(Expr::Function { name, args })
    }

    fn parse_exists(&mut self) -> Result<Expr> {
        self.advance(); // consume Exists
        self.expect(Token::LParen)?;
        let subquery = self.parse_select()?;
        self.expect(Token::RParen)?;
        // For v1, just treat as a function call
        Ok(Expr::Function {
            name: "exists".to_string(),
            args: vec![Expr::Literal(LiteralValue::String(format!("{:?}", subquery)))],
        })
    }

    fn make_number_literal(&self, s: &str) -> Result<Expr> {
        if s.contains('.') || s.contains('e') || s.contains('E') {
            let val: f64 = s
                .parse()
                .map_err(|_| SQLError::Syntax(format!("invalid float: {}", s)))?;
            Ok(Expr::Literal(LiteralValue::Float(val)))
        } else {
            let val: i64 = s
                .parse()
                .map_err(|_| SQLError::Syntax(format!("invalid integer: {}", s)))?;
            Ok(Expr::Literal(LiteralValue::Integer(val)))
        }
    }

    fn parse_identifier(&mut self) -> Result<String> {
        match self.peek() {
            Token::Identifier(s) => {
                self.advance();
                Ok(s)
            }
            t => Err(SQLError::Syntax(format!(
                "expected identifier, got {:?}",
                t
            ))),
        }
    }

    fn parse_literal(&mut self) -> Result<LiteralValue> {
        if self.eat_if(Token::Null) {
            return Ok(LiteralValue::Null);
        }
        if self.eat_if(Token::True) {
            return Ok(LiteralValue::Boolean(true));
        }
        if self.eat_if(Token::False) {
            return Ok(LiteralValue::Boolean(false));
        }
        if let Token::Number(s) = self.peek() {
            self.advance();
            if s.contains('.') || s.contains('e') || s.contains('E') {
                let val: f64 = s
                    .parse()
                    .map_err(|_| SQLError::Syntax(format!("invalid float: {}", s)))?;
                return Ok(LiteralValue::Float(val));
            }
            let val: i64 = s
                .parse()
                .map_err(|_| SQLError::Syntax(format!("invalid integer: {}", s)))?;
            return Ok(LiteralValue::Integer(val));
        }
        if let Token::String(s) = self.peek() {
            self.advance();
            return Ok(LiteralValue::String(s));
        }
        Err(SQLError::Syntax(format!(
            "unexpected token in literal: {:?}",
            self.peek()
        )))
    }

    fn parse_usize(&mut self) -> Result<usize> {
        match self.peek() {
            Token::Number(s) => {
                self.advance();
                s.parse()
                    .map_err(|_| SQLError::Syntax(format!("invalid number: {}", s)))
            }
            t => Err(SQLError::Syntax(format!("expected number, got {:?}", t))),
        }
    }

    // Helpers

    fn peek(&self) -> Token {
        self.tokens
            .get(self.pos)
            .cloned()
            .unwrap_or(Token::EOF)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        let actual = self.peek();
        if actual == expected {
            self.advance();
            Ok(())
        } else {
            Err(SQLError::Syntax(format!(
                "expected {:?}, got {:?}",
                expected, actual
            )))
        }
    }

    fn eat_if(&mut self, expected: Token) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek_is_identifier(&self) -> bool {
        matches!(self.peek(), Token::Identifier(_))
    }

    fn parse_comma_separated<T>(
        &mut self,
        mut parser: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut items = Vec::new();
        items.push(parser(self)?);
        while self.eat_if(Token::Comma) {
            items.push(parser(self)?);
        }
        Ok(items)
    }
}
