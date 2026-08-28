use crate::ast::{BinaryOperator, Block, Declaration, Expression, FunctionDeclaration, Program};
use crate::error::{Error, SourcePosition};
use crate::lexer::{Token, TokenKind};

/// Maximum recursive nesting depth the parser accepts before refusing input.
///
/// Kept well below the thread stack budget: each nesting level recurses
/// through the whole expression-precedence chain (about eight frames per
/// level), so an overly deep limit would overflow the stack on adversarial
/// input instead of reporting a clear error.
const MAX_DEPTH: usize = 128;

pub(crate) struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
            depth: 0,
        }
    }

    pub(crate) fn parse(mut self) -> Result<Program, Error> {
        let functions = self.parse_functions()?;
        let declarations = self.parse_declarations()?;

        if self.tokens.is_empty() || self.peek().is_none() {
            if declarations.is_empty() {
                return Err(Error::new("expression is empty"));
            }
            return Err(Error::new("expected a final expression after declarations"));
        }

        let expression = self.parse_expression()?;

        if let Some(token) = self.peek() {
            if token.kind == TokenKind::RightParen {
                return Err(Error::at("unmatched ')'", token.position));
            }
            if token.kind == TokenKind::RightBrace {
                return Err(Error::at("unexpected '}'", token.position));
            }
            return Err(Error::at(
                format!("unexpected trailing token: {}", token.kind_name()),
                token.position,
            ));
        }

        Ok(Program {
            functions,
            declarations,
            expression,
        })
    }

    /// Parses zero or more `fn name(param, ...) = <body>;` declarations that
    /// precede the `let` declarations and final expression. Every function is
    /// bound before any body or call is parsed so functions are visible to all
    /// declarations, the final expression, and recursive calls from their own
    /// bodies.
    fn parse_functions(&mut self) -> Result<Vec<FunctionDeclaration>, Error> {
        let mut functions = Vec::new();

        while matches!(self.peek_kind(), Some(TokenKind::Fn)) {
            functions.push(self.parse_function(&functions)?);
        }

        Ok(functions)
    }

    fn parse_function(
        &mut self,
        existing: &[FunctionDeclaration],
    ) -> Result<FunctionDeclaration, Error> {
        let function_position = self.peek().map(|token| token.position);
        self.advance(); // consume 'fn'

        let name = match self.advance() {
            Some(Token {
                kind: TokenKind::Identifier(name),
                ..
            }) => name.clone(),
            Some(token) => {
                return Err(Error::at(
                    "expected a function name after 'fn'",
                    token.position,
                ))
            }
            None => return Err(self.error_here("expected a function name after 'fn'")),
        };

        if existing.iter().any(|function| function.name == name) {
            return Err(Error::at(
                format!("duplicate function declaration: '{name}'"),
                function_position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        let parameters = self.parse_parameters(&name)?;
        let body = self.parse_function_body()?;

        let semicolon = self.advance();
        if !matches!(
            semicolon,
            Some(Token {
                kind: TokenKind::Semicolon,
                ..
            })
        ) {
            let position = semicolon.map(|token| token.position).or(function_position);
            return Err(Error::at(
                format!("expected ';' after declaration of function '{name}'"),
                position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        Ok(FunctionDeclaration {
            name,
            parameters,
            body,
            position: function_position,
        })
    }

    /// Parses the `(param, ...)` parameter list of a function. Parameters are
    /// plain identifiers; duplicates are rejected within the single list.
    fn parse_parameters(&mut self, name: &str) -> Result<Vec<String>, Error> {
        let open = self.advance();
        if !matches!(
            open,
            Some(Token {
                kind: TokenKind::LeftParen,
                ..
            })
        ) {
            let position = open
                .map(|token| token.position)
                .or_else(|| self.peek().map(|token| token.position));
            return Err(Error::at(
                format!("expected '(' after function name '{name}'"),
                position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        let mut parameters = Vec::new();
        loop {
            // An empty list `()` or a trailing comma are not supported. Every
            // iteration either breaks on `)` or consumes one parameter plus an
            // optional trailing comma, so it always makes progress.
            if self.peek().is_none() {
                return Err(self.error_here("unmatched '(' in function parameter list"));
            }
            if matches!(self.peek_kind(), Some(TokenKind::RightParen)) {
                self.advance();
                break;
            }

            match self.advance() {
                Some(Token {
                    kind: TokenKind::Identifier(param),
                    position,
                }) => {
                    let param = param.clone();
                    if parameters.contains(&param) {
                        return Err(Error::at(
                            format!("duplicate parameter name in function '{name}': '{param}'"),
                            *position,
                        ));
                    }
                    parameters.push(param);
                    if matches!(self.peek_kind(), Some(TokenKind::RightParen)) {
                        self.advance();
                        break;
                    }
                    if matches!(self.peek_kind(), Some(TokenKind::Comma)) {
                        self.advance();
                    } else {
                        return Err(self.error_here(format!(
                            "expected ',' or ')' in function '{name}' parameter list"
                        )));
                    }
                }
                Some(token) => {
                    return Err(Error::at(
                        format!("expected a parameter name after '(' in function '{name}'"),
                        token.position,
                    ))
                }
                None => return Err(self.error_here("unmatched '(' in function parameter list")),
            }
        }

        Ok(parameters)
    }

    /// Parses a `(arg, ...)` call after an identifier callee has already been
    /// consumed. Trailing commas are not supported.
    fn parse_call(
        &mut self,
        callee: String,
        position: Option<SourcePosition>,
    ) -> Result<Expression, Error> {
        self.advance(); // consume '('

        let mut arguments = Vec::new();
        loop {
            if self.peek().is_none() {
                return Err(Error::at(
                    "unmatched '(' in function call",
                    position.unwrap_or(SourcePosition { line: 1, column: 1 }),
                ));
            }
            if matches!(self.peek_kind(), Some(TokenKind::RightParen)) {
                self.advance();
                break;
            }
            if matches!(self.peek_kind(), Some(TokenKind::Comma)) {
                return Err(Error::at(
                    "unexpected ',' in function call",
                    self.peek()
                        .map(|t| t.position)
                        .unwrap_or(SourcePosition { line: 1, column: 1 }),
                ));
            }

            arguments.push(self.parse_expression()?);
            if matches!(self.peek_kind(), Some(TokenKind::RightParen)) {
                self.advance();
                break;
            }
            if matches!(self.peek_kind(), Some(TokenKind::Comma)) {
                self.advance();
            } else {
                return Err(self.error_here("expected ',' or ')' in function call arguments"));
            }
        }

        Ok(Expression::Call {
            callee,
            arguments,
            position,
        })
    }

    /// Parses the block body of a function after its `=`. Function bodies are
    /// blocks just like `if`/`else` branches: `{ declaration* expression }`.
    fn parse_function_body(&mut self) -> Result<Block, Error> {
        let equals = self.advance();
        if !matches!(
            equals,
            Some(Token {
                kind: TokenKind::Equals,
                ..
            })
        ) {
            let position = equals
                .map(|token| token.position)
                .or_else(|| self.peek().map(|token| token.position));
            return Err(Error::at(
                "expected '=' before the function body",
                position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        if !matches!(self.peek_kind(), Some(TokenKind::LeftBrace)) {
            return match self.peek() {
                Some(token) => Err(Error::at(
                    "expected a block for the function body",
                    token.position,
                )),
                None => Err(Error::new("expected a block for the function body")),
            };
        }

        self.parse_block()
    }

    /// Parses zero or more `let` declarations that belong to the current
    /// scope (the program top level or one block).
    fn parse_declarations(&mut self) -> Result<Vec<Declaration>, Error> {
        let mut declarations = Vec::new();

        while matches!(self.peek_kind(), Some(TokenKind::Let)) {
            declarations.push(self.parse_declaration(&declarations)?);
        }

        Ok(declarations)
    }

    fn parse_declaration(&mut self, existing: &[Declaration]) -> Result<Declaration, Error> {
        let declaration_position = self.peek().map(|token| token.position);
        self.advance(); // consume 'let'

        let name = match self.advance() {
            Some(Token {
                kind: TokenKind::Identifier(name),
                ..
            }) => name.clone(),
            Some(token) => {
                return Err(Error::at(
                    "expected a variable name after 'let'",
                    token.position,
                ))
            }
            None => return Err(self.error_here("expected a variable name after 'let'")),
        };

        let equals = self.advance();
        if !matches!(
            equals,
            Some(Token {
                kind: TokenKind::Equals,
                ..
            })
        ) {
            let position = equals.map(|token| token.position).or(declaration_position);
            return Err(Error::at(
                format!("expected '=' after variable name '{name}'"),
                position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        let initializer = self.parse_expression()?;

        let semicolon = self.advance();
        if !matches!(
            semicolon,
            Some(Token {
                kind: TokenKind::Semicolon,
                ..
            })
        ) {
            let position = semicolon
                .map(|token| token.position)
                .or(declaration_position);
            return Err(Error::at(
                format!("expected ';' after declaration of '{name}'"),
                position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        if existing
            .iter()
            .any(|declaration: &Declaration| declaration.name == name)
        {
            return Err(Error::at(
                format!("duplicate variable declaration: '{name}'"),
                declaration_position.unwrap_or(SourcePosition { line: 1, column: 1 }),
            ));
        }

        Ok(Declaration {
            name,
            initializer,
            position: declaration_position,
        })
    }

    fn parse_expression(&mut self) -> Result<Expression, Error> {
        self.enter()?;
        let expression = self.parse_or();
        self.leave();
        expression
    }

    fn parse_or(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_and()?;

        while matches!(self.peek_kind(), Some(TokenKind::OrOr)) {
            let position = self.peek().map(|token| token.position);
            self.advance();
            let right = self.parse_and()?;
            expression = Expression::LogicalOr {
                left: Box::new(expression),
                right: Box::new(right),
                position,
            };
        }

        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_comparison()?;

        while matches!(self.peek_kind(), Some(TokenKind::AndAnd)) {
            let position = self.peek().map(|token| token.position);
            self.advance();
            let right = self.parse_comparison()?;
            expression = Expression::LogicalAnd {
                left: Box::new(expression),
                right: Box::new(right),
                position,
            };
        }

        Ok(expression)
    }

    fn parse_comparison(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_additive()?;

        if let Some(operator) = self.comparison_operator() {
            self.advance();
            let right = self.parse_additive()?;
            let position = expression.position();
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
                position,
            };

            if self.comparison_operator().is_some() {
                return Err(self.error_here("comparison operators cannot be chained"));
            }
        }

        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_multiplicative()?;

        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Plus) => BinaryOperator::Add,
                Some(TokenKind::Minus) => BinaryOperator::Subtract,
                _ => break,
            };
            let position = self.peek().map(|token| token.position);
            self.advance();
            let right = self.parse_multiplicative()?;
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
                position,
            };
        }

        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_unary()?;

        loop {
            let operator = match self.peek_kind() {
                Some(TokenKind::Star) => BinaryOperator::Multiply,
                Some(TokenKind::Slash) => BinaryOperator::Divide,
                Some(TokenKind::Percent) => BinaryOperator::Remainder,
                _ => break,
            };
            let position = self.peek().map(|token| token.position);
            self.advance();
            let right = self.parse_unary()?;
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
                position,
            };
        }

        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expression, Error> {
        let operator = match self.peek_kind() {
            Some(TokenKind::Minus) => Some(UnaryOperator::Negation),
            Some(TokenKind::Bang) => Some(UnaryOperator::Not),
            _ => None,
        };

        if let Some(operator) = operator {
            let position = self.peek().map(|token| token.position);
            self.advance();
            let operand = self.parse_unary_nested()?;
            let expression = match operator {
                UnaryOperator::Negation => Expression::UnaryNegation {
                    operand: Box::new(operand),
                    position,
                },
                UnaryOperator::Not => Expression::UnaryNot {
                    operand: Box::new(operand),
                    position,
                },
            };
            return Ok(expression);
        }

        self.parse_primary()
    }

    fn parse_unary_nested(&mut self) -> Result<Expression, Error> {
        self.enter()?;
        let expression = self.parse_unary();
        self.leave();
        expression
    }

    fn parse_primary(&mut self) -> Result<Expression, Error> {
        // Handle `if` before the advancing match: the If arm calls a
        // `&mut self` method, which is not allowed while the match
        // scrutinee's borrow of `self` is still alive.
        if matches!(self.peek_kind(), Some(TokenKind::If)) {
            let position = self
                .peek()
                .map(|token| token.position)
                .expect("the 'if' token was peeked above");
            self.advance();
            return self.parse_if_expression(position);
        }

        match self.advance() {
            Some(Token {
                kind: TokenKind::Integer(value),
                position,
            }) => Ok(Expression::Literal {
                value: *value,
                position: Some(*position),
            }),
            Some(Token {
                kind: TokenKind::String(value),
                position,
            }) => Ok(Expression::StringLiteral {
                value: value.clone(),
                position: Some(*position),
            }),
            Some(Token {
                kind: TokenKind::True,
                position,
            }) => Ok(Expression::BoolLiteral {
                value: true,
                position: Some(*position),
            }),
            Some(Token {
                kind: TokenKind::False,
                position,
            }) => Ok(Expression::BoolLiteral {
                value: false,
                position: Some(*position),
            }),
            Some(Token {
                kind: TokenKind::Identifier(name),
                position,
            }) => {
                // Copy the owned values out of the token before borrowing `self`
                // again, so the `match self.advance()` borrow is released.
                let name = name.clone();
                let position = Some(*position);
                // An identifier followed by `(` is a function call; otherwise
                // it is a plain variable reference.
                if matches!(self.peek_kind(), Some(TokenKind::LeftParen)) {
                    return self.parse_call(name, position);
                }
                Ok(Expression::Variable { name, position })
            }
            Some(Token {
                kind: TokenKind::LeftParen,
                ..
            }) => {
                let expression = self.parse_expression()?;
                match self.advance() {
                    Some(Token {
                        kind: TokenKind::RightParen,
                        ..
                    }) => Ok(expression),
                    Some(token) => Err(Error::at("unmatched '('", token.position)),
                    None => Err(self.error_here("unmatched '('")),
                }
            }
            Some(token) => match &token.kind {
                TokenKind::RightParen => Err(Error::at("unexpected ')'", token.position)),
                TokenKind::RightBrace => Err(Error::at("unexpected '}'", token.position)),
                _ => Err(Error::at("expected an expression", token.position)),
            },
            None => Err(self.error_here("expected an expression")),
        }
    }

    fn parse_if_expression(&mut self, position: SourcePosition) -> Result<Expression, Error> {
        let condition = self.parse_expression()?;
        let then_branch = self.parse_if_block("after if condition")?;

        match self.advance() {
            Some(Token {
                kind: TokenKind::Else,
                ..
            }) => {}
            Some(token) => {
                return Err(Error::at(
                    "expected 'else' after the if branch",
                    token.position,
                ))
            }
            None => return Err(self.error_here("expected 'else' after the if branch")),
        }

        let else_branch = self.parse_if_block("after 'else'")?;

        Ok(Expression::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
            position: Some(position),
        })
    }

    /// Parses the `{ declaration* expression }` block that follows an `if`
    /// condition or an `else`. `context` names the offending position in the
    /// error message when the block is missing.
    fn parse_if_block(&mut self, context: &str) -> Result<Block, Error> {
        if !matches!(self.peek_kind(), Some(TokenKind::LeftBrace)) {
            return match self.peek() {
                Some(token) => Err(Error::at(
                    format!("expected a block {context}"),
                    token.position,
                )),
                None => Err(Error::new(format!("expected a block {context}"))),
            };
        }

        self.parse_block()
    }

    fn parse_block(&mut self) -> Result<Block, Error> {
        self.advance(); // consume '{'

        let declarations = self.parse_declarations()?;
        let expression = self.parse_block_expression(&declarations)?;

        match self.advance() {
            Some(Token {
                kind: TokenKind::RightBrace,
                ..
            }) => Ok(Block {
                declarations,
                expression,
            }),
            Some(token) => Err(Error::at("expected '}' to close the block", token.position)),
            None => Err(self.error_here("expected '}' to close the block")),
        }
    }

    /// Parses the final expression of a block, which is terminated by `}`
    /// rather than the end of input.
    fn parse_block_expression(
        &mut self,
        declarations: &[Declaration],
    ) -> Result<Expression, Error> {
        if self.peek().is_none() || matches!(self.peek_kind(), Some(TokenKind::RightBrace)) {
            if declarations.is_empty() {
                return Err(self.error_here("expression is empty"));
            }
            return Err(self.error_here("expected a final expression after declarations"));
        }

        self.parse_expression()
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::new("program too deeply nested"));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|token| &token.kind)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    fn error_here(&self, message: impl Into<String>) -> Error {
        match self.peek() {
            Some(token) => Error::at(message, token.position),
            None => Error::new(message),
        }
    }

    fn comparison_operator(&self) -> Option<BinaryOperator> {
        match self.peek_kind() {
            Some(TokenKind::LessThan) => Some(BinaryOperator::LessThan),
            Some(TokenKind::LessThanOrEqual) => Some(BinaryOperator::LessThanOrEqual),
            Some(TokenKind::GreaterThan) => Some(BinaryOperator::GreaterThan),
            Some(TokenKind::GreaterThanOrEqual) => Some(BinaryOperator::GreaterThanOrEqual),
            Some(TokenKind::EqualEqual) => Some(BinaryOperator::Equal),
            Some(TokenKind::NotEqual) => Some(BinaryOperator::NotEqual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UnaryOperator {
    Negation,
    Not,
}

#[cfg(test)]
mod tests {
    use super::{Parser, MAX_DEPTH};
    use crate::lexer::Lexer;

    fn parse_error(input: &str) -> String {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(&tokens).parse().unwrap_err().to_string()
    }

    fn parses_ok(input: &str) -> bool {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(&tokens).parse().is_ok()
    }

    fn parse_error_positions(input: &str) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();
        let tokens = Lexer::new(input).tokenize().unwrap();
        match Parser::new(&tokens).parse() {
            Ok(_) => {}
            Err(error) => {
                positions.push(
                    error
                        .position()
                        .map(|position| (position.line, position.column))
                        .unwrap_or((0, 0)),
                );
            }
        }
        positions
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse_error("   "), "expression is empty");
    }

    #[test]
    fn rejects_modulo_with_a_missing_operand() {
        assert_eq!(parse_error("1 %"), "expected an expression");
        assert_eq!(parse_error("1 % * 2"), "expected an expression");
    }

    #[test]
    fn rejects_malformed_syntax() {
        assert_eq!(parse_error("1 + * 2"), "expected an expression");
        assert_eq!(parse_error("+1"), "expected an expression");
        assert_eq!(parse_error("1 < = 2"), "expected an expression");
        assert_eq!(parse_error("1 > = 2"), "expected an expression");
    }

    #[test]
    fn reports_the_position_of_an_error_within_a_line() {
        // The offending '*' where an expression was expected is at line 1,
        // column 5 of "1 + * 2".
        assert_eq!(parse_error_positions("1 + * 2"), vec![(1, 5)]);
    }

    #[test]
    fn reports_the_position_of_an_error_across_lines() {
        // The trailing '3' at line 2, column 5 is the offending token.
        assert_eq!(parse_error_positions("1 +\n  2 3"), vec![(2, 5)]);
    }

    #[test]
    fn rejects_chained_comparisons() {
        assert_eq!(
            parse_error("1 < 2 < 3"),
            "comparison operators cannot be chained"
        );
        assert_eq!(
            parse_error("1 < 2 == 1"),
            "comparison operators cannot be chained"
        );
    }

    #[test]
    fn rejects_unmatched_parentheses() {
        assert_eq!(parse_error("(1 + 2"), "unmatched '('");
        assert_eq!(parse_error("1 + 2)"), "unmatched ')'");
    }

    #[test]
    fn rejects_trailing_input() {
        assert_eq!(
            parse_error("1 2"),
            "unexpected trailing token: integer literal"
        );
        assert_eq!(parse_error("1 = 2"), "unexpected trailing token: '='");
        assert_eq!(
            parse_error("let value = 1;"),
            "expected a final expression after declarations"
        );
        assert_eq!(
            parse_error("let value = 1; value;"),
            "unexpected trailing token: ';'"
        );
    }

    #[test]
    fn rejects_missing_declaration_parts() {
        assert_eq!(
            parse_error("let value 1; value"),
            "expected '=' after variable name 'value'"
        );
        assert_eq!(
            parse_error("let value = 1 value"),
            "expected ';' after declaration of 'value'"
        );
        assert_eq!(
            parse_error("let = 1; 1"),
            "expected a variable name after 'let'"
        );
        assert_eq!(
            parse_error("let + 1"),
            "expected a variable name after 'let'"
        );
    }

    #[test]
    fn rejects_duplicate_variables() {
        assert_eq!(
            parse_error("let value = 1; let value = 2; value"),
            "duplicate variable declaration: 'value'"
        );
    }

    #[test]
    fn accepts_deep_parentheses_within_the_limit() {
        let depth = 100;
        let input = format!("{}{}{}", "(".repeat(depth), "1", ")".repeat(depth));
        assert!(parses_ok(&input));
    }

    #[test]
    fn rejects_parentheses_nesting_beyond_the_limit() {
        let depth = MAX_DEPTH + 1;
        let input = format!("{}{}{}", "(".repeat(depth), "1", ")".repeat(depth));
        assert_eq!(parse_error(&input), "program too deeply nested");
    }

    #[test]
    fn rejects_unary_minus_chains_beyond_the_limit() {
        let depth = MAX_DEPTH + 1;
        let input = format!("{}1", "-".repeat(depth));
        assert_eq!(parse_error(&input), "program too deeply nested");
    }

    #[test]
    fn accepts_unary_minus_chains_within_the_limit() {
        let depth = 100;
        let input = format!("{}1", "-".repeat(depth));
        assert!(parses_ok(&input));
    }

    #[test]
    fn accepts_boolean_literals_and_logical_operators() {
        for program in [
            "true",
            "false",
            "!true",
            "!!false",
            "true && false",
            "true || false",
            "true && false || !true",
            "1 < 2 && 2 < 3",
            "let ready = 3 >= 2; ready && true",
        ] {
            assert!(parses_ok(program), "program: {program}");
        }
    }

    #[test]
    fn accepts_string_literals_and_concatenation() {
        for program in [
            r#""hello""#,
            r#""a" + "b" + "c""#,
            r#"let greeting = "hi"; greeting == "hi""#,
        ] {
            assert!(parses_ok(program), "program: {program}");
        }
    }

    #[test]
    fn accepts_if_expressions_with_blocks() {
        for program in [
            "if true { 1 } else { 2 }",
            "if 1 < 2 { \"a\" } else { \"b\" }",
            "let x = if true { 1 } else { 2 }; x",
            "if true { let x = 1; x } else { let y = 2; y }",
            "let x = 1; if true { let x = 2; x } else { x }",
            "if true { if false { 1 } else { 2 } } else { 3 }",
            "if (1 < 2) && (2 < 3) { 1 } else { 2 }",
        ] {
            assert!(parses_ok(program), "program: {program}");
        }
    }

    #[test]
    fn rejects_if_expressions_with_missing_parts() {
        assert_eq!(
            parse_error("if true { 1 }"),
            "expected 'else' after the if branch"
        );
        assert_eq!(
            parse_error("if true 1 else 2"),
            "expected a block after if condition"
        );
        assert_eq!(
            parse_error("if true { 1 } else 2"),
            "expected a block after 'else'"
        );
        assert_eq!(
            parse_error("if true"),
            "expected a block after if condition"
        );
        assert_eq!(parse_error("if { 1 } else { 2 }"), "expected an expression");
    }

    #[test]
    fn rejects_blocks_without_a_final_expression() {
        assert_eq!(parse_error("if true {} else { 2 }"), "expression is empty");
        assert_eq!(
            parse_error("if true { let x = 1; } else { 2 }"),
            "expected a final expression after declarations"
        );
    }

    #[test]
    fn rejects_unclosed_blocks() {
        assert_eq!(
            parse_error("if true { 1"),
            "expected '}' to close the block"
        );
    }

    #[test]
    fn rejects_stray_braces() {
        assert_eq!(parse_error("1 }"), "unexpected '}'");
        assert_eq!(parse_error("{ 1 }"), "expected an expression");
    }

    #[test]
    fn rejects_duplicate_variables_within_a_block_but_allows_shadowing() {
        assert_eq!(
            parse_error("if true { let x = 1; let x = 2; x } else { 1 }"),
            "duplicate variable declaration: 'x'"
        );
        assert!(parses_ok("let x = 1; if true { let x = 2; x } else { x }"));
    }
}
