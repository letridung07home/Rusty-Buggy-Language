use crate::ast::{BinaryOperator, Declaration, Expression, Program};
use crate::error::Error;
use crate::lexer::Token;

pub(crate) struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    pub(crate) fn parse(mut self) -> Result<Program, Error> {
        let mut declarations = Vec::new();

        while matches!(self.peek(), Some(Token::Let)) {
            self.advance();

            let name = match self.advance() {
                Some(Token::Identifier(name)) => name.to_owned(),
                _ => return Err(Error::new("expected a variable name after 'let'")),
            };

            if !matches!(self.advance(), Some(Token::Equals)) {
                return Err(Error::new(format!(
                    "expected '=' after variable name '{name}'"
                )));
            }

            let initializer = self.parse_expression()?;

            if !matches!(self.advance(), Some(Token::Semicolon)) {
                return Err(Error::new(format!(
                    "expected ';' after declaration of '{name}'"
                )));
            }

            if declarations
                .iter()
                .any(|declaration: &Declaration| declaration.name == name)
            {
                return Err(Error::new(format!(
                    "duplicate variable declaration: '{name}'"
                )));
            }

            declarations.push(Declaration { name, initializer });
        }

        if self.tokens.is_empty() || self.peek().is_none() {
            if declarations.is_empty() {
                return Err(Error::new("expression is empty"));
            }
            return Err(Error::new("expected a final expression after declarations"));
        }

        let expression = self.parse_expression()?;

        if let Some(token) = self.peek() {
            if *token == Token::RightParen {
                return Err(Error::new("unmatched ')'"));
            }
            return Err(Error::new(format!(
                "unexpected trailing token: {}",
                token.name()
            )));
        }

        Ok(Program {
            declarations,
            expression,
        })
    }

    fn parse_expression(&mut self) -> Result<Expression, Error> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_additive()?;

        if let Some(operator) = self.comparison_operator() {
            self.advance();
            let right = self.parse_additive()?;
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
            };

            if self.comparison_operator().is_some() {
                return Err(Error::new("comparison operators cannot be chained"));
            }
        }

        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_multiplicative()?;

        loop {
            let operator = match self.peek() {
                Some(Token::Plus) => BinaryOperator::Add,
                Some(Token::Minus) => BinaryOperator::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }

        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, Error> {
        let mut expression = self.parse_unary()?;

        loop {
            let operator = match self.peek() {
                Some(Token::Star) => BinaryOperator::Multiply,
                Some(Token::Slash) => BinaryOperator::Divide,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }

        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expression, Error> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.advance();
            let expression = self.parse_unary()?;
            return Ok(Expression::UnaryNegation(Box::new(expression)));
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, Error> {
        match self.advance() {
            Some(Token::Integer(value)) => Ok(Expression::Literal(*value)),
            Some(Token::Identifier(name)) => Ok(Expression::Variable(name.to_owned())),
            Some(Token::LeftParen) => {
                let expression = self.parse_expression()?;
                match self.advance() {
                    Some(Token::RightParen) => Ok(expression),
                    _ => Err(Error::new("unmatched '('")),
                }
            }
            Some(Token::RightParen) => Err(Error::new("unexpected ')'")),
            Some(Token::Let)
            | Some(Token::Equals)
            | Some(Token::LessThan)
            | Some(Token::LessThanOrEqual)
            | Some(Token::GreaterThan)
            | Some(Token::GreaterThanOrEqual)
            | Some(Token::EqualEqual)
            | Some(Token::NotEqual)
            | Some(Token::Semicolon)
            | Some(Token::Plus)
            | Some(Token::Minus)
            | Some(Token::Star)
            | Some(Token::Slash) => Err(Error::new("expected an expression")),
            None => Err(Error::new("expected an expression")),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    fn comparison_operator(&self) -> Option<BinaryOperator> {
        match self.peek() {
            Some(Token::LessThan) => Some(BinaryOperator::LessThan),
            Some(Token::LessThanOrEqual) => Some(BinaryOperator::LessThanOrEqual),
            Some(Token::GreaterThan) => Some(BinaryOperator::GreaterThan),
            Some(Token::GreaterThanOrEqual) => Some(BinaryOperator::GreaterThanOrEqual),
            Some(Token::EqualEqual) => Some(BinaryOperator::Equal),
            Some(Token::NotEqual) => Some(BinaryOperator::NotEqual),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;
    use crate::lexer::Lexer;

    fn parse_error(input: &str) -> String {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(&tokens).parse().unwrap_err().to_string()
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
}
