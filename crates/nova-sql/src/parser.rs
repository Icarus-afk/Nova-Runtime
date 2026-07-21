use crate::ast::*;
use crate::error::{Result, SQLError};
use crate::lexer::Token;

const MAX_NESTING_DEPTH: usize = 64;

pub struct Parser {
    tokens: Vec<Token>,
    positions: Vec<(usize, usize)>,
    pos: usize,
    depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, positions: Vec<(usize, usize)>) -> Self {
        Parser {
            tokens,
            positions,
            pos: 0,
            depth: 0,
        }
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
            t => Err(self.err_syntax(format!(
                "unexpected token at start of statement: {:?}",
                t
            ))),
        }
    }

    fn parse_select(&mut self) -> Result<SelectStatement> {
        self.expect(Token::Select)?;
        let distinct = self.eat_if(Token::Distinct);
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
            distinct,
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
        let nulls_first = if self.eat_if(Token::Nulls) {
            if self.eat_if(Token::First) {
                Some(true)
            } else if self.eat_if(Token::Last) {
                Some(false)
            } else {
                return Err(self.err_syntax("expected FIRST or LAST after NULLS".to_string()));
            }
        } else {
            None
        };
        Ok(OrderByExpr { expr, asc, nulls_first })
    }

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
        let mut nullable = true;
        let mut unique = false;
        let mut is_primary_key = false;
        let mut default = None;
        let mut auto_increment = false;
        let mut check_expr = None;

        loop {
            if self.eat_if(Token::Not) {
                self.expect(Token::Null)?;
                nullable = false;
            } else if self.eat_if(Token::Null) {
                nullable = true;
            } else if self.eat_if(Token::Default) {
                if self.eat_identifier("current_timestamp")
                    || self.eat_identifier("current_date")
                    || self.eat_identifier("current_time")
                {
                    default = Some(LiteralValue::String("current_timestamp".to_string()));
                } else {
                    default = Some(self.parse_literal()?);
                }
            } else if self.eat_if(Token::Primary) {
                self.expect(Token::Key)?;
                is_primary_key = true;
                nullable = false;
            } else if self.eat_if(Token::Unique) {
                unique = true;
            } else if self.eat_identifier("auto_increment") {
                auto_increment = true;
            } else if self.eat_if(Token::Check) {
                self.expect(Token::LParen)?;
                check_expr = Some(self.parse_expression()?);
                self.expect(Token::RParen)?;
            } else {
                break;
            }
        }
        Ok(ColumnDef {
            name,
            sql_type,
            nullable,
            default,
            unique,
            is_primary_key,
            auto_increment,
            check_expr,
        })
    }

    fn parse_sql_type(&mut self) -> Result<SQLType> {
        let ident = self.parse_identifier()?;
        match ident.to_lowercase().as_str() {
            "int" | "integer" | "tinyint" | "smallint" | "mediumint" | "bigint" => Ok(SQLType::Integer),
            "float" | "double" | "real" | "decimal" | "numeric" => Ok(SQLType::Float),
            "text" | "varchar" | "string" | "char" | "tinytext" | "mediumtext" | "longtext" => Ok(SQLType::Text),
            "bool" | "boolean" | "bit" => Ok(SQLType::Boolean),
            "timestamp" | "datetime" | "date" | "time" | "year" => Ok(SQLType::Text),
            "blob" | "tinyblob" | "mediumblob" | "longblob" | "binary" | "varbinary" => Ok(SQLType::Text),
            other => Err(self.err_syntax(format!("unknown type: {}", other))),
        }
    }

    fn parse_drop_table(&mut self) -> Result<DropTableStatement> {
        self.expect(Token::Drop)?;
        self.expect(Token::Table)?;
        let table = self.parse_table_ref()?;
        Ok(DropTableStatement { table })
    }

    fn parse_expression(&mut self) -> Result<Expr> {
        if self.depth >= MAX_NESTING_DEPTH {
            return Err(SQLError::QueryTooComplex(
                "max nesting depth exceeded".to_string(),
            ));
        }
        self.depth += 1;
        let result = self.parse_or();
        self.depth -= 1;
        result
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
        if self.eat_if(Token::Case) {
            return self.parse_case();
        }
        if self.eat_if(Token::Cast) {
            return self.parse_cast();
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
                let args = self.parse_comma_separated(Self::parse_expression)?;
                self.expect(Token::RParen)?;
                return Ok(Expr::Function { name, args });
            }
            // Handle :: cast syntax
            if self.eat_if(Token::ColonColon) {
                let target_type = self.parse_sql_type()?;
                return Ok(Expr::Cast {
                    expr: Box::new(Expr::Column(name)),
                    target_type,
                });
            }
            let mut expr = Expr::Column(name);
            // Handle NOT IN / NOT BETWEEN / NOT LIKE / NOT ILIKE
            let saved = self.pos;
            if self.eat_if(Token::Not) {
                if self.eat_if(Token::In) {
                    self.expect(Token::LParen)?;
                    let list = self.parse_comma_separated(Self::parse_expression)?;
                    self.expect(Token::RParen)?;
                    expr = Expr::UnaryOp {
                        op: UnaryOperator::Not,
                        expr: Box::new(Expr::In { expr: Box::new(expr), list }),
                    };
                } else if self.eat_if(Token::Between) {
                    let low = self.parse_comparison()?;
                    self.expect(Token::And)?;
                    let high = self.parse_expression()?;
                    let between = Expr::Between { expr: Box::new(expr), low: Box::new(low), high: Box::new(high) };
                    expr = Expr::UnaryOp { op: UnaryOperator::Not, expr: Box::new(between) };
                } else if self.eat_if(Token::Like) {
                    let pattern = self.parse_expression()?;
                    let like = Expr::Like { expr: Box::new(expr), pattern: Box::new(pattern) };
                    expr = Expr::UnaryOp { op: UnaryOperator::Not, expr: Box::new(like) };
                } else if self.eat_if(Token::ILike) {
                    let pattern = self.parse_expression()?;
                    let ilike = Expr::ILike { expr: Box::new(expr), pattern: Box::new(pattern) };
                    expr = Expr::UnaryOp { op: UnaryOperator::Not, expr: Box::new(ilike) };
                } else {
                    self.pos = saved;
                    // Check IS / IN / BETWEEN / LIKE / ILIKE normally
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
                        expr = Expr::In { expr: Box::new(expr), list };
                    } else if self.eat_if(Token::Between) {
                        let low = self.parse_comparison()?;
                        self.expect(Token::And)?;
                        let high = self.parse_expression()?;
                        expr = Expr::Between { expr: Box::new(expr), low: Box::new(low), high: Box::new(high) };
                    } else if self.eat_if(Token::Like) {
                        let pattern = self.parse_expression()?;
                        expr = Expr::Like { expr: Box::new(expr), pattern: Box::new(pattern) };
                    } else if self.eat_if(Token::ILike) {
                        let pattern = self.parse_expression()?;
                        expr = Expr::ILike { expr: Box::new(expr), pattern: Box::new(pattern) };
                    }
                }
            } else if self.eat_if(Token::Is) {
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
                let low = self.parse_comparison()?;
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
            } else if self.eat_if(Token::ILike) {
                let pattern = self.parse_expression()?;
                expr = Expr::ILike {
                    expr: Box::new(expr),
                    pattern: Box::new(pattern),
                };
            }
            return Ok(expr);
        }
        Err(self.err_syntax(format!(
            "unexpected token: {:?}",
            self.peek()
        )))
    }

    fn parse_case(&mut self) -> Result<Expr> {
        let mut whens = Vec::new();
        let mut else_val = None;
        loop {
            if self.eat_if(Token::When) {
                let cond = self.parse_expression()?;
                self.expect(Token::Then)?;
                let result = self.parse_expression()?;
                whens.push((cond, result));
            } else if self.eat_if(Token::Else) {
                else_val = Some(Box::new(self.parse_expression()?));
            } else if self.eat_if(Token::End) {
                break;
            } else {
                return Err(self.err_syntax(
                    "expected WHEN, ELSE, or END in CASE expression".to_string(),
                ));
            }
        }
        if whens.is_empty() {
            return Err(self.err_syntax("CASE must have at least one WHEN".to_string()));
        }
        Ok(Expr::Case { whens, else_val })
    }

    fn parse_cast(&mut self) -> Result<Expr> {
        self.expect(Token::LParen)?;
        let expr = self.parse_expression()?;
        self.expect(Token::As)?;
        let target_type = self.parse_sql_type()?;
        self.expect(Token::RParen)?;
        Ok(Expr::Cast {
            expr: Box::new(expr),
            target_type,
        })
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
            _ => {
                return Err(self.err_syntax("expected aggregate function".to_string()));
            }
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
        self.advance();
        self.expect(Token::LParen)?;
        let subquery = self.parse_select()?;
        self.expect(Token::RParen)?;
        Ok(Expr::Function {
            name: "exists".to_string(),
            args: vec![Expr::Literal(LiteralValue::String(format!("{:?}", subquery)))],
        })
    }

    fn make_number_literal(&self, s: &str) -> Result<Expr> {
        if s.contains('.') || s.contains('e') || s.contains('E') {
            let val: f64 = s
                .parse()
                .map_err(|_| SQLError::syntax(format!("invalid float: {}", s)))?;
            Ok(Expr::Literal(LiteralValue::Float(val)))
        } else {
            let val: i64 = s
                .parse()
                .map_err(|_| SQLError::syntax(format!("invalid integer: {}", s)))?;
            Ok(Expr::Literal(LiteralValue::Integer(val)))
        }
    }

    fn parse_identifier(&mut self) -> Result<String> {
        match self.peek() {
            Token::Identifier(s) => {
                self.advance();
                Ok(s)
            }
            t => Err(self.err_syntax(format!(
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
                    .map_err(|_| SQLError::syntax(format!("invalid float: {}", s)))?;
                return Ok(LiteralValue::Float(val));
            }
            let val: i64 = s
                .parse()
                .map_err(|_| SQLError::syntax(format!("invalid integer: {}", s)))?;
            return Ok(LiteralValue::Integer(val));
        }
        if let Token::String(s) = self.peek() {
            self.advance();
            return Ok(LiteralValue::String(s));
        }
        Err(self.err_syntax(format!(
            "unexpected token in literal: {:?}",
            self.peek()
        )))
    }

    fn parse_usize(&mut self) -> Result<usize> {
        match self.peek() {
            Token::Number(s) => {
                self.advance();
                s.parse()
                    .map_err(|_| SQLError::syntax(format!("invalid number: {}", s)))
            }
            t => Err(self.err_syntax(format!("expected number, got {:?}", t))),
        }
    }

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
            Err(self.err_syntax(format!(
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

    fn eat_identifier(&mut self, ident: &str) -> bool {
        match self.peek() {
            Token::Identifier(ref s) if s.eq_ignore_ascii_case(ident) => {
                self.advance();
                true
            }
            _ => false,
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

    fn err_syntax(&self, msg: String) -> SQLError {
        let (start, end) = self.positions.get(self.pos).copied().unwrap_or((0, 0));
        SQLError::Syntax { message: msg, start, end }
    }
}
