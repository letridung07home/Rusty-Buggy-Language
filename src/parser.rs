use crate::ast::{BinaryOperator, Declaration, Expression, Program};
use crate::error::{Error, SourcePosition};
use crate::lexer::{Token, TokenKind};

/// Maximum recursive nesting depth the parser accepts before refusing input.
/// This is intentionally far below the thread stack budget so adversarial
/// input reports a clear error instead of overflowing the stack.
const MAX_DEPTH: usize = 256;

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
        let mut declarations = Vec::new();

        while matches!(self.peek_kind(), Some(TokenKind::Let)) {
            let declaration_position = self.peek().map(|token| token.position);
            self.advance();

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

            if declarations
                .iter()
                .any(|declaration: &Declaration| declaration.name == name)
            {
                return Err(Error::at(
                    format!("duplicate variable declaration: '{name}'"),
                    declaration_position.unwrap_or(SourcePosition { line: 1, column: 1 }),
                ));
            }

            declarations.push(Declaration {
                name,
                initializer,
                position: declaration_position,
            });
        }

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
            return Err(Error::at(
                format!("unexpected trailing token: {}", token.kind_name()),
                token.position,
            ));
        }

        Ok(Program {
            declarations,
            expression,
        })
    }

    fn parse_expression(&mut self) -> Result<Expression, Error> {
        self.enter()?;
        let expression = self.parse_comparison();
        self.leave();
        expression
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
        if matches!(self.peek_kind(), Some(TokenKind::Minus)) {
            let position = self.peek().map(|token| token.position);
            self.advance();
            let operand = self.parse_unary_nested()?;
            return Ok(Expression::UnaryNegation {
                operand: Box::new(operand),
                position,
            });
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
        match self.advance() {
            Some(Token {
                kind: TokenKind::Integer(value),
                position,
            }) => Ok(Expression::Literal {
                value: *value,
                position: Some(*position),
            }),
            Some(Token {
                kind: TokenKind::Identifier(name),
                position,
            }) => Ok(Expression::Variable {
                name: name.clone(),
                position: Some(*position),
            }),
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
                TokenKind::Let
                | TokenKind::Equals
                | TokenKind::LessThan
                | TokenKind::LessThanOrEqual
                | TokenKind::GreaterThan
                | TokenKind::GreaterThanOrEqual
                | TokenKind::EqualEqual
                | TokenKind::NotEqual
                | TokenKind::Semicolon
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash => Err(Error::at("expected an expression", token.position)),
                TokenKind::Integer(_) | TokenKind::Identifier(_) | TokenKind::LeftParen => {
                    // Unreachable: these arms are handled above.
                    Err(Error::at("expected an expression", token.position))
                }
            },
            None => Err(self.error_here("expected an expression")),
        }
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
        let depth = 200;
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
}
