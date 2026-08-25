use std::collections::HashMap;

use crate::ast::{BinaryOperator, Expression, Program};
use crate::error::{Error, SourcePosition};

pub(crate) fn evaluate(program: &Program) -> Result<i64, Error> {
    let mut variables = HashMap::new();

    for declaration in &program.declarations {
        let value = evaluate_expression(&declaration.initializer, &variables)?;
        variables.insert(declaration.name.clone(), value);
    }

    evaluate_expression(&program.expression, &variables)
}

fn evaluate_expression(
    expression: &Expression,
    variables: &HashMap<String, i64>,
) -> Result<i64, Error> {
    match expression {
        Expression::Literal { value, position } => i64::try_from(*value)
            .map_err(|_| positioned_error("integer literal out of range", *position)),
        Expression::Variable { name, position } => variables
            .get(name)
            .copied()
            .ok_or_else(|| positioned_error(format!("undefined variable: '{name}'"), *position)),
        Expression::UnaryNegation { operand, position } => {
            if let Expression::Literal { value, .. } = operand.as_ref() {
                if *value == (i64::MAX as u64) + 1 {
                    return Ok(i64::MIN);
                }
            }

            evaluate_expression(operand, variables)?
                .checked_neg()
                .ok_or_else(|| positioned_error("integer negation overflow", *position))
        }
        Expression::Binary {
            operator,
            left,
            right,
            position,
        } => {
            let left = evaluate_expression(left, variables)?;
            let right = evaluate_expression(right, variables)?;

            match operator {
                BinaryOperator::Add => left
                    .checked_add(right)
                    .ok_or_else(|| positioned_error("integer addition overflow", *position)),
                BinaryOperator::Subtract => left
                    .checked_sub(right)
                    .ok_or_else(|| positioned_error("integer subtraction overflow", *position)),
                BinaryOperator::Multiply => left
                    .checked_mul(right)
                    .ok_or_else(|| positioned_error("integer multiplication overflow", *position)),
                BinaryOperator::Divide => {
                    if right == 0 {
                        Err(positioned_error("division by zero", *position))
                    } else {
                        left.checked_div(right)
                            .ok_or_else(|| positioned_error("integer division overflow", *position))
                    }
                }
                BinaryOperator::Remainder => {
                    if right == 0 {
                        Err(positioned_error("division by zero", *position))
                    } else {
                        left.checked_rem(right).ok_or_else(|| {
                            positioned_error("integer remainder overflow", *position)
                        })
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

fn positioned_error(message: impl Into<String>, position: Option<SourcePosition>) -> Error {
    match position {
        Some(position) => Error::at(message, position),
        None => Error::new(message),
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate;
    use crate::error::Error;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn evaluate_source(input: &str) -> Result<i64, Error> {
        let tokens = Lexer::new(input).tokenize()?;
        let program = Parser::new(&tokens).parse()?;
        evaluate(&program)
    }

    #[test]
    fn respects_operator_precedence() {
        assert_eq!(evaluate_source("1 + 2 * 3"), Ok(7));
        assert_eq!(evaluate_source("(1 + 2) * 3"), Ok(9));
        assert_eq!(evaluate_source("-2 * 3 + 4"), Ok(-2));
    }

    #[test]
    fn preserves_associativity_and_whitespace_behavior() {
        assert_eq!(evaluate_source("10 - 3 - 2"), Ok(5));
        assert_eq!(evaluate_source("20 / 5 / 2"), Ok(2));
        assert_eq!(evaluate_source(" \t12\n / 5 "), Ok(2));
    }

    #[test]
    fn evaluates_signed_values_and_checked_arithmetic() {
        assert_eq!(evaluate_source("1 - 3 * 2"), Ok(-5));
        assert_eq!(evaluate_source("0 - 7 / 3"), Ok(-2));
        assert_eq!(evaluate_source("-1"), Ok(-1));
        assert_eq!(evaluate_source("-(1 + 2)"), Ok(-3));
        assert_eq!(evaluate_source("3 * -2"), Ok(-6));
        assert_eq!(evaluate_source("-2 * -3"), Ok(6));
        assert_eq!(evaluate_source("--1"), Ok(1));
        assert_eq!(evaluate_source("---1"), Ok(-1));
    }

    #[test]
    fn evaluates_comparisons_as_integer_values() {
        assert_eq!(evaluate_source("1 < 2"), Ok(1));
        assert_eq!(evaluate_source("2 < 1"), Ok(0));
        assert_eq!(evaluate_source("2 <= 2"), Ok(1));
        assert_eq!(evaluate_source("3 <= 2"), Ok(0));
        assert_eq!(evaluate_source("2 > 1"), Ok(1));
        assert_eq!(evaluate_source("1 > 2"), Ok(0));
        assert_eq!(evaluate_source("2 >= 2"), Ok(1));
        assert_eq!(evaluate_source("1 >= 2"), Ok(0));
        assert_eq!(evaluate_source("2 == 2"), Ok(1));
        assert_eq!(evaluate_source("2 == 3"), Ok(0));
        assert_eq!(evaluate_source("2 != 3"), Ok(1));
        assert_eq!(evaluate_source("2 != 2"), Ok(0));
        assert_eq!(evaluate_source("1 + 2 < 2 * 2"), Ok(1));
        assert_eq!(evaluate_source("10 - 3 >= 2 + 6"), Ok(0));
        assert_eq!(evaluate_source("(1 < 2) == (2 < 3)"), Ok(1));
        assert_eq!(evaluate_source("(1 + 2 < 4) * 5"), Ok(5));
    }

    #[test]
    fn accepts_the_signed_minimum_literal() {
        assert_eq!(evaluate_source("-9223372036854775808"), Ok(i64::MIN));
        assert_eq!(evaluate_source("-(9223372036854775808)"), Ok(i64::MIN));
        assert_eq!(
            evaluate_source("0 - 9223372036854775807 - 1 / (0 - 1)"),
            Ok(-9223372036854775806)
        );
    }

    #[test]
    fn reports_evaluation_errors_without_changing_their_messages() {
        assert_eq!(
            evaluate_source("9223372036854775807 + 1")
                .unwrap_err()
                .to_string(),
            "integer addition overflow"
        );
        assert_eq!(
            evaluate_source("9223372036854775808")
                .unwrap_err()
                .to_string(),
            "integer literal out of range"
        );
        assert_eq!(
            evaluate_source("-9223372036854775809")
                .unwrap_err()
                .to_string(),
            "integer literal out of range"
        );
        assert_eq!(
            evaluate_source("8 / (3 - 3)").unwrap_err().to_string(),
            "division by zero"
        );
        assert_eq!(
            evaluate_source("--9223372036854775808")
                .unwrap_err()
                .to_string(),
            "integer negation overflow"
        );
        assert_eq!(
            evaluate_source("-9223372036854775808 / -1")
                .unwrap_err()
                .to_string(),
            "integer division overflow"
        );
        assert_eq!(
            evaluate_source("missing + 1").unwrap_err().to_string(),
            "undefined variable: 'missing'"
        );
        assert_eq!(
            evaluate_source("let first = second; let second = 2; first")
                .unwrap_err()
                .to_string(),
            "undefined variable: 'second'"
        );
    }

    #[test]
    fn evaluates_modulo_with_truncating_division_semantics() {
        assert_eq!(evaluate_source("10 % 3"), Ok(1));
        assert_eq!(evaluate_source("20 % 5"), Ok(0));
        assert_eq!(evaluate_source("-7 % 3"), Ok(-1));
        assert_eq!(evaluate_source("7 % -3"), Ok(1));
        assert_eq!(evaluate_source("-7 % -3"), Ok(-1));
    }

    #[test]
    fn evaluates_modulo_at_multiplicative_precedence() {
        assert_eq!(evaluate_source("2 + 3 * 4 % 5"), Ok(4));
        assert_eq!(evaluate_source("(2 + 3) % 4"), Ok(1));
        assert_eq!(evaluate_source("17 % 5 * 2"), Ok(4));
        assert_eq!(evaluate_source("20 % 6 % 5"), Ok(2));
    }

    #[test]
    fn evaluates_modulo_in_variable_declarations() {
        assert_eq!(evaluate_source("let a = 10; let b = a % 3; b + 1"), Ok(2));
    }

    #[test]
    fn reports_modulo_errors_with_positions() {
        assert_eq!(
            evaluate_source("8 % (3 - 3)").unwrap_err().to_string(),
            "division by zero"
        );
        assert_eq!(
            evaluate_source("-9223372036854775808 % -1")
                .unwrap_err()
                .to_string(),
            "integer remainder overflow"
        );

        // The '%' operator sits at line 1, column 3 of "8 % (3 - 3)".
        let error = evaluate_source("8 % (3 - 3)").unwrap_err();
        assert_eq!(
            error.position(),
            Some(crate::error::SourcePosition {
                line: 1,
                column: 3
            })
        );
    }

    #[test]
    fn evaluates_immutable_variables_in_order() {
        assert_eq!(
            evaluate_source("let rate = 20; let quantity = 5; rate * quantity"),
            Ok(100)
        );
        assert_eq!(
            evaluate_source("let first = 2; let second = first + 3; second * 4"),
            Ok(20)
        );
        assert_eq!(evaluate_source("let ready = 3 >= 2; ready + 4"), Ok(5));
        assert_eq!(evaluate_source("let _value2 = 7; _value2"), Ok(7));
    }
}
