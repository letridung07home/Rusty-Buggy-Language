#[derive(Debug, PartialEq)]
enum Token {
    Integer(u64),
    Identifier(String),
    Let,
    Equals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    EqualEqual,
    NotEqual,
    Semicolon,
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

            if character.is_ascii_alphabetic() || character == '_' {
                tokens.push(self.identifier());
                continue;
            }

            let token = match character {
                '=' => self.operator_with_optional_equals(Token::Equals, Token::EqualEqual),
                '<' => self.operator_with_optional_equals(Token::LessThan, Token::LessThanOrEqual),
                '>' => self
                    .operator_with_optional_equals(Token::GreaterThan, Token::GreaterThanOrEqual),
                '!' => {
                    self.advance();
                    if self.current_character() == Some('=') {
                        self.advance();
                        Token::NotEqual
                    } else {
                        return Err("unexpected character '!'".to_owned());
                    }
                }
                ';' => {
                    self.advance();
                    Token::Semicolon
                }
                '+' => {
                    self.advance();
                    Token::Plus
                }
                '-' => {
                    self.advance();
                    Token::Minus
                }
                '*' => {
                    self.advance();
                    Token::Star
                }
                '/' => {
                    self.advance();
                    Token::Slash
                }
                '(' => {
                    self.advance();
                    Token::LeftParen
                }
                ')' => {
                    self.advance();
                    Token::RightParen
                }
                _ => return Err(format!("unexpected character '{character}'")),
            };
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn operator_with_optional_equals(&mut self, single: Token, compound: Token) -> Token {
        self.advance();
        if self.current_character() == Some('=') {
            self.advance();
            compound
        } else {
            single
        }
    }

    fn integer_literal(&mut self) -> Result<Token, String> {
        let mut value = 0_u64;

        while let Some(character) = self.current_character() {
            if !character.is_ascii_digit() {
                break;
            }

            let digit = u64::from(character as u8 - b'0');
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| "integer literal out of range".to_owned())?;
            self.advance();
        }

        Ok(Token::Integer(value))
    }

    fn identifier(&mut self) -> Token {
        let start = self.position;

        while let Some(character) = self.current_character() {
            if !character.is_ascii_alphanumeric() && character != '_' {
                break;
            }
            self.advance();
        }

        let name = &self.input[start..self.position];
        if name == "let" {
            Token::Let
        } else {
            Token::Identifier(name.to_owned())
        }
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
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, PartialEq)]
enum Expression {
    Literal(u64),
    Variable(String),
    UnaryNegation(Box<Expression>),
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

#[derive(Debug, PartialEq)]
struct Declaration {
    name: String,
    initializer: Expression,
}

#[derive(Debug, PartialEq)]
struct Program {
    declarations: Vec<Declaration>,
    expression: Expression,
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

    fn parse(mut self) -> Result<Program, String> {
        let mut declarations = Vec::new();

        while matches!(self.peek(), Some(Token::Let)) {
            self.advance();

            let name = match self.advance() {
                Some(Token::Identifier(name)) => name.to_owned(),
                _ => return Err("expected a variable name after 'let'".to_owned()),
            };

            if !matches!(self.advance(), Some(Token::Equals)) {
                return Err(format!("expected '=' after variable name '{name}'"));
            }

            let initializer = self.parse_expression()?;

            if !matches!(self.advance(), Some(Token::Semicolon)) {
                return Err(format!("expected ';' after declaration of '{name}'"));
            }

            if declarations
                .iter()
                .any(|declaration: &Declaration| declaration.name == name)
            {
                return Err(format!("duplicate variable declaration: '{name}'"));
            }

            declarations.push(Declaration { name, initializer });
        }

        if self.tokens.is_empty() || self.peek().is_none() {
            if declarations.is_empty() {
                return Err("expression is empty".to_owned());
            }
            return Err("expected a final expression after declarations".to_owned());
        }

        let expression = self.parse_expression()?;

        if let Some(token) = self.peek() {
            if *token == Token::RightParen {
                return Err("unmatched ')'".to_owned());
            }
            return Err(format!("unexpected trailing token: {}", token.name()));
        }

        Ok(Program {
            declarations,
            expression,
        })
    }

    fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expression, String> {
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
                return Err("comparison operators cannot be chained".to_owned());
            }
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

    fn parse_unary(&mut self) -> Result<Expression, String> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.advance();
            let expression = self.parse_unary()?;
            return Ok(Expression::UnaryNegation(Box::new(expression)));
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, String> {
        match self.advance() {
            Some(Token::Integer(value)) => Ok(Expression::Literal(*value)),
            Some(Token::Identifier(name)) => Ok(Expression::Variable(name.to_owned())),
            Some(Token::LeftParen) => {
                let expression = self.parse_expression()?;
                match self.advance() {
                    Some(Token::RightParen) => Ok(expression),
                    _ => Err("unmatched '('".to_owned()),
                }
            }
            Some(Token::RightParen) => Err("unexpected ')'".to_owned()),
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
            | Some(Token::Slash) => Err("expected an expression".to_owned()),
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

impl Token {
    fn name(&self) -> &'static str {
        match self {
            Self::Integer(_) => "integer literal",
            Self::Identifier(_) => "identifier",
            Self::Let => "'let'",
            Self::Equals => "'='",
            Self::LessThan => "'<'",
            Self::LessThanOrEqual => "'<='",
            Self::GreaterThan => "'>'",
            Self::GreaterThanOrEqual => "'>='",
            Self::EqualEqual => "'=='",
            Self::NotEqual => "'!='",
            Self::Semicolon => "';'",
            Self::Plus => "'+'",
            Self::Minus => "'-'",
            Self::Star => "'*'",
            Self::Slash => "'/'",
            Self::LeftParen => "'('",
            Self::RightParen => "')'",
        }
    }
}

fn evaluate_expression(
    expression: &Expression,
    variables: &std::collections::HashMap<String, i64>,
) -> Result<i64, String> {
    match expression {
        Expression::Literal(value) => {
            i64::try_from(*value).map_err(|_| "integer literal out of range".to_owned())
        }
        Expression::Variable(name) => variables
            .get(name)
            .copied()
            .ok_or_else(|| format!("undefined variable: '{name}'")),
        Expression::UnaryNegation(operand) => {
            if let Expression::Literal(value) = operand.as_ref() {
                if *value == (i64::MAX as u64) + 1 {
                    return Ok(i64::MIN);
                }
            }

            evaluate_expression(operand, variables)?
                .checked_neg()
                .ok_or_else(|| "integer negation overflow".to_owned())
        }
        Expression::Binary {
            operator,
            left,
            right,
        } => {
            let left = evaluate_expression(left, variables)?;
            let right = evaluate_expression(right, variables)?;

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
                BinaryOperator::LessThan => Ok(if left < right { 1 } else { 0 }),
                BinaryOperator::LessThanOrEqual => Ok(if left <= right { 1 } else { 0 }),
                BinaryOperator::GreaterThan => Ok(if left > right { 1 } else { 0 }),
                BinaryOperator::GreaterThanOrEqual => Ok(if left >= right { 1 } else { 0 }),
                BinaryOperator::Equal => Ok(if left == right { 1 } else { 0 }),
                BinaryOperator::NotEqual => Ok(if left != right { 1 } else { 0 }),
            }
        }
    }
}

pub(super) fn evaluate(input: &str) -> Result<i64, String> {
    let tokens = Lexer::new(input).tokenize()?;
    let program = Parser::new(&tokens).parse()?;
    let mut variables = std::collections::HashMap::new();

    for declaration in &program.declarations {
        let value = evaluate_expression(&declaration.initializer, &variables)?;
        variables.insert(declaration.name.clone(), value);
    }

    evaluate_expression(&program.expression, &variables)
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    #[test]
    fn respects_operator_precedence() {
        assert_eq!(evaluate("1 + 2 * 3"), Ok(7));
    }

    #[test]
    fn comparisons_return_one_or_zero() {
        assert_eq!(evaluate("1 < 2"), Ok(1));
        assert_eq!(evaluate("2 < 1"), Ok(0));
        assert_eq!(evaluate("2 <= 2"), Ok(1));
        assert_eq!(evaluate("3 <= 2"), Ok(0));
        assert_eq!(evaluate("2 > 1"), Ok(1));
        assert_eq!(evaluate("1 > 2"), Ok(0));
        assert_eq!(evaluate("2 >= 2"), Ok(1));
        assert_eq!(evaluate("1 >= 2"), Ok(0));
        assert_eq!(evaluate("2 == 2"), Ok(1));
        assert_eq!(evaluate("2 == 3"), Ok(0));
        assert_eq!(evaluate("2 != 3"), Ok(1));
        assert_eq!(evaluate("2 != 2"), Ok(0));
    }

    #[test]
    fn comparisons_have_lower_precedence_than_arithmetic() {
        assert_eq!(evaluate("1 + 2 < 2 * 2"), Ok(1));
        assert_eq!(evaluate("10 - 3 >= 2 + 6"), Ok(0));
    }

    #[test]
    fn comparisons_can_be_parenthesized() {
        assert_eq!(evaluate("(1 < 2) == (2 < 3)"), Ok(1));
        assert_eq!(evaluate("(1 + 2 < 4) * 5"), Ok(5));
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
    fn rejects_standalone_comparison_punctuation() {
        assert_eq!(
            evaluate("1 ! 2"),
            Err("unexpected character '!'".to_owned())
        );
        assert_eq!(
            evaluate("1 = 2"),
            Err("unexpected trailing token: '='".to_owned())
        );
        assert_eq!(
            evaluate("1 < = 2"),
            Err("expected an expression".to_owned())
        );
        assert_eq!(
            evaluate("1 > = 2"),
            Err("expected an expression".to_owned())
        );
    }

    #[test]
    fn rejects_chained_comparisons() {
        assert_eq!(
            evaluate("1 < 2 < 3"),
            Err("comparison operators cannot be chained".to_owned())
        );
        assert_eq!(
            evaluate("1 < 2 == 1"),
            Err("comparison operators cannot be chained".to_owned())
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
    fn accepts_negative_literals() {
        assert_eq!(evaluate("-1"), Ok(-1));
    }

    #[test]
    fn negates_parenthesized_expressions() {
        assert_eq!(evaluate("-(1 + 2)"), Ok(-3));
    }

    #[test]
    fn multiplies_negative_operands() {
        assert_eq!(evaluate("3 * -2"), Ok(-6));
        assert_eq!(evaluate("-2 * -3"), Ok(6));
    }

    #[test]
    fn unary_negation_is_right_associative() {
        assert_eq!(evaluate("--1"), Ok(1));
        assert_eq!(evaluate("---1"), Ok(-1));
    }

    #[test]
    fn unary_negation_binds_tighter_than_multiplication() {
        assert_eq!(evaluate("-2 * 3 + 4"), Ok(-2));
    }

    #[test]
    fn accepts_i64_minimum_as_a_negated_literal() {
        assert_eq!(evaluate("-9223372036854775808"), Ok(i64::MIN));
        assert_eq!(evaluate("-(9223372036854775808)"), Ok(i64::MIN));
    }

    #[test]
    fn rejects_out_of_range_literal_magnitudes() {
        assert_eq!(
            evaluate("9223372036854775808"),
            Err("integer literal out of range".to_owned())
        );
        assert_eq!(
            evaluate("-9223372036854775809"),
            Err("integer literal out of range".to_owned())
        );
    }

    #[test]
    fn rejects_unary_plus() {
        assert_eq!(evaluate("+1"), Err("expected an expression".to_owned()));
        assert_eq!(evaluate("1 * +2"), Err("expected an expression".to_owned()));
    }

    #[test]
    fn rejects_negation_overflow() {
        assert_eq!(
            evaluate("--9223372036854775808"),
            Err("integer negation overflow".to_owned())
        );
    }

    #[test]
    fn rejects_negated_minimum_divided_by_negative_one() {
        assert_eq!(
            evaluate("-9223372036854775808 / -1"),
            Err("integer division overflow".to_owned())
        );
    }

    #[test]
    fn evaluates_immutable_variables() {
        assert_eq!(
            evaluate("let rate = 20; let quantity = 5; rate * quantity"),
            Ok(100)
        );
    }

    #[test]
    fn declaration_initializers_see_only_previous_bindings() {
        assert_eq!(
            evaluate("let first = 2; let second = first + 3; second * 4"),
            Ok(20)
        );
    }

    #[test]
    fn stores_comparison_values_in_variables() {
        assert_eq!(evaluate("let ready = 3 >= 2; ready + 4"), Ok(5));
    }

    #[test]
    fn supports_identifiers_with_digits_and_underscores() {
        assert_eq!(evaluate("let _value2 = 7; _value2"), Ok(7));
    }

    #[test]
    fn rejects_undefined_variables() {
        assert_eq!(
            evaluate("missing + 1"),
            Err("undefined variable: 'missing'".to_owned())
        );
    }

    #[test]
    fn rejects_forward_references() {
        assert_eq!(
            evaluate("let first = second; let second = 2; first"),
            Err("undefined variable: 'second'".to_owned())
        );
    }

    #[test]
    fn rejects_duplicate_variables() {
        assert_eq!(
            evaluate("let value = 1; let value = 2; value"),
            Err("duplicate variable declaration: 'value'".to_owned())
        );
    }

    #[test]
    fn rejects_missing_declaration_parts() {
        assert_eq!(
            evaluate("let value 1; value"),
            Err("expected '=' after variable name 'value'".to_owned())
        );
        assert_eq!(
            evaluate("let value = 1 value"),
            Err("expected ';' after declaration of 'value'".to_owned())
        );
        assert_eq!(
            evaluate("let = 1; 1"),
            Err("expected a variable name after 'let'".to_owned())
        );
    }

    #[test]
    fn rejects_programs_without_a_final_expression() {
        assert_eq!(
            evaluate("let value = 1;"),
            Err("expected a final expression after declarations".to_owned())
        );
    }

    #[test]
    fn rejects_trailing_semicolons() {
        assert_eq!(
            evaluate("let value = 1; value;"),
            Err("unexpected trailing token: ';'".to_owned())
        );
    }

    #[test]
    fn keeps_let_reserved() {
        assert_eq!(
            evaluate("let + 1"),
            Err("expected a variable name after 'let'".to_owned())
        );
    }
}
