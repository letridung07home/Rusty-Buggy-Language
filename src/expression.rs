#[derive(Debug, PartialEq)]
enum Token {
    Integer(i64),
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
}

struct Lexer<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while let Some(character) = self.current_character() {
            if character.is_ascii_whitespace() {
                self.advance();
                continue;
            }

            if character.is_ascii_digit() {
                tokens.push(self.integer_literal()?);
                continue;
            }

            let token = match character {
                '+' => Token::Plus,
                '-' => Token::Minus,
                '*' => Token::Star,
                '/' => Token::Slash,
                '(' => Token::LeftParen,
                ')' => Token::RightParen,
                _ => return Err(format!("unexpected character '{character}'")),
            };
            self.advance();
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn integer_literal(&mut self) -> Result<Token, String> {
        let mut value = 0_i64;

        while let Some(character) = self.current_character() {
            if !character.is_ascii_digit() {
                break;
            }

            let digit = i64::from(character as u8 - b'0');
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| "integer literal out of range".to_owned())?;
            self.advance();
        }

        Ok(Token::Integer(value))
    }

    fn current_character(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.current_character() {
            self.position += character.len_utf8();
        }
    }
}

#[derive(Debug, PartialEq)]
enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, PartialEq)]
enum Expression {
    Literal(i64),
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse(mut self) -> Result<Expression, String> {
        if self.tokens.is_empty() {
            return Err("expression is empty".to_owned());
        }

        let expression = self.parse_additive()?;

        if let Some(token) = self.peek() {
            if *token == Token::RightParen {
                return Err("unmatched ')'".to_owned());
            }
            return Err(format!("unexpected trailing token: {}", token.name()));
        }

        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<Expression, String> {
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

    fn parse_multiplicative(&mut self) -> Result<Expression, String> {
        let mut expression = self.parse_primary()?;

        loop {
            let operator = match self.peek() {
                Some(Token::Star) => BinaryOperator::Multiply,
                Some(Token::Slash) => BinaryOperator::Divide,
                _ => break,
            };
            self.advance();
            let right = self.parse_primary()?;
            expression = Expression::Binary {
                operator,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }

        Ok(expression)
    }

    fn parse_primary(&mut self) -> Result<Expression, String> {
        match self.advance() {
            Some(Token::Integer(value)) => Ok(Expression::Literal(*value)),
            Some(Token::LeftParen) => {
                let expression = self.parse_additive()?;
                match self.advance() {
                    Some(Token::RightParen) => Ok(expression),
                    _ => Err("unmatched '('".to_owned()),
                }
            }
            Some(Token::RightParen) => Err("unexpected ')'".to_owned()),
            Some(Token::Plus) | Some(Token::Minus) | Some(Token::Star) | Some(Token::Slash) => {
                Err("expected an expression".to_owned())
            }
            None => Err("expected an expression".to_owned()),
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
}

impl Token {
    fn name(&self) -> &'static str {
        match self {
            Self::Integer(_) => "integer literal",
            Self::Plus => "'+'",
            Self::Minus => "'-'",
            Self::Star => "'*'",
            Self::Slash => "'/'",
            Self::LeftParen => "'('",
            Self::RightParen => "')'",
        }
    }
}

fn evaluate_expression(expression: &Expression) -> Result<i64, String> {
    match expression {
        Expression::Literal(value) => Ok(*value),
        Expression::Binary {
            operator,
            left,
            right,
        } => {
            let left = evaluate_expression(left)?;
            let right = evaluate_expression(right)?;

            match operator {
                BinaryOperator::Add => left
                    .checked_add(right)
                    .ok_or_else(|| "integer addition overflow".to_owned()),
                BinaryOperator::Subtract => left
                    .checked_sub(right)
                    .ok_or_else(|| "integer subtraction overflow".to_owned()),
                BinaryOperator::Multiply => left
                    .checked_mul(right)
                    .ok_or_else(|| "integer multiplication overflow".to_owned()),
                BinaryOperator::Divide => {
                    if right == 0 {
                        Err("division by zero".to_owned())
                    } else {
                        left.checked_div(right)
                            .ok_or_else(|| "integer division overflow".to_owned())
                    }
                }
            }
        }
    }
}

pub(super) fn evaluate(input: &str) -> Result<i64, String> {
    let tokens = Lexer::new(input).tokenize()?;
    let expression = Parser::new(&tokens).parse()?;
    evaluate_expression(&expression)
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    #[test]
    fn respects_operator_precedence() {
        assert_eq!(evaluate("1 + 2 * 3"), Ok(7));
    }

    #[test]
    fn respects_parentheses() {
        assert_eq!(evaluate("(1 + 2) * 3"), Ok(9));
    }

    #[test]
    fn subtraction_is_left_associative() {
        assert_eq!(evaluate("10 - 3 - 2"), Ok(5));
    }

    #[test]
    fn division_is_left_associative() {
        assert_eq!(evaluate("20 / 5 / 2"), Ok(2));
    }

    #[test]
    fn accepts_whitespace() {
        assert_eq!(evaluate(" \t12\n / 5 "), Ok(2));
    }

    #[test]
    fn supports_negative_computed_results() {
        assert_eq!(evaluate("1 - 3 * 2"), Ok(-5));
    }

    #[test]
    fn division_truncates_toward_zero() {
        assert_eq!(evaluate("0 - 7 / 3"), Ok(-2));
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(evaluate("   "), Err("expression is empty".to_owned()));
    }

    #[test]
    fn rejects_malformed_syntax() {
        assert_eq!(
            evaluate("1 + * 2"),
            Err("expected an expression".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_characters() {
        assert_eq!(
            evaluate("1 @ 2"),
            Err("unexpected character '@'".to_owned())
        );
    }

    #[test]
    fn rejects_unmatched_parentheses() {
        assert_eq!(evaluate("(1 + 2"), Err("unmatched '('".to_owned()));
        assert_eq!(evaluate("1 + 2)"), Err("unmatched ')'".to_owned()));
    }

    #[test]
    fn rejects_trailing_input() {
        assert_eq!(
            evaluate("1 2"),
            Err("unexpected trailing token: integer literal".to_owned())
        );
    }

    #[test]
    fn rejects_out_of_range_literals() {
        assert_eq!(
            evaluate("9223372036854775808"),
            Err("integer literal out of range".to_owned())
        );
    }

    #[test]
    fn rejects_arithmetic_overflow() {
        assert_eq!(
            evaluate("9223372036854775807 + 1"),
            Err("integer addition overflow".to_owned())
        );
    }

    #[test]
    fn rejects_signed_minimum_divided_by_negative_one() {
        assert_eq!(
            evaluate("0 - 9223372036854775807 - 1 / (0 - 1)"),
            Ok(-9223372036854775806)
        );
        assert_eq!(
            evaluate("(0 - 9223372036854775807 - 1) / (0 - 1)"),
            Err("integer division overflow".to_owned())
        );
    }

    #[test]
    fn rejects_division_by_zero() {
        assert_eq!(evaluate("8 / (3 - 3)"), Err("division by zero".to_owned()));
    }

    #[test]
    fn unary_negation_is_not_supported() {
        assert_eq!(evaluate("-1"), Err("expected an expression".to_owned()));
    }
}
