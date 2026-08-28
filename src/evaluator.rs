use std::collections::HashMap;

use crate::ast::{BinaryOperator, Block, Declaration, Expression, Program};
use crate::error::{Error, SourcePosition};
use crate::Value;

pub(crate) fn evaluate(program: &Program) -> Result<Value, Error> {
    let mut scopes = vec![HashMap::new()];
    evaluate_body(&program.declarations, &program.expression, &mut scopes)
}

/// Evaluates declarations into the current innermost scope and then the final
/// expression, mirroring the type checker's scope discipline.
fn evaluate_body(
    declarations: &[Declaration],
    expression: &Expression,
    scopes: &mut Vec<HashMap<String, Value>>,
) -> Result<Value, Error> {
    for declaration in declarations {
        let value = evaluate_expression(&declaration.initializer, scopes)?;
        let current = scopes.last_mut().expect("the scope stack is never empty");
        current.insert(declaration.name.clone(), value);
    }
    evaluate_expression(expression, scopes)
}

fn evaluate_block(block: &Block, scopes: &mut Vec<HashMap<String, Value>>) -> Result<Value, Error> {
    scopes.push(HashMap::new());
    let result = evaluate_body(&block.declarations, &block.expression, scopes);
    scopes.pop();
    result
}

fn evaluate_expression(
    expression: &Expression,
    scopes: &mut Vec<HashMap<String, Value>>,
) -> Result<Value, Error> {
    match expression {
        Expression::Literal { value, position } => i64::try_from(*value)
            .map(Value::Int)
            .map_err(|_| positioned_error("integer literal out of range", *position)),
        Expression::StringLiteral { value, .. } => Ok(Value::String(value.clone())),
        Expression::BoolLiteral { value, .. } => Ok(Value::Bool(*value)),
        Expression::Variable { name, position } => {
            for scope in scopes.iter().rev() {
                if let Some(value) = scope.get(name) {
                    return Ok(value.clone());
                }
            }
            Err(positioned_error(
                format!("undefined variable: '{name}'"),
                *position,
            ))
        }
        Expression::UnaryNegation { operand, position } => {
            // The magnitude 2^63 has no unnegated representation; a literal of
            // that magnitude is only valid under an immediately applied `-`.
            if let Expression::Literal { value, .. } = operand.as_ref() {
                if *value == (i64::MAX as u64) + 1 {
                    return Ok(Value::Int(i64::MIN));
                }
            }

            match evaluate_expression(operand, scopes)? {
                Value::Int(value) => value
                    .checked_neg()
                    .map(Value::Int)
                    .ok_or_else(|| positioned_error("integer negation overflow", *position)),
                other => Err(positioned_error(
                    format!(
                        "type mismatch in '-': expected an integer, found {}",
                        value_type_name(&other)
                    ),
                    *position,
                )),
            }
        }
        Expression::UnaryNot { operand, position } => match evaluate_expression(operand, scopes)? {
            Value::Bool(value) => Ok(Value::Bool(!value)),
            other => Err(positioned_error(
                format!(
                    "type mismatch in '!': expected a boolean, found {}",
                    value_type_name(&other)
                ),
                *position,
            )),
        },
        Expression::Binary {
            operator,
            left,
            right,
            position,
        } => {
            let left_value = evaluate_expression(left, scopes)?;
            let right_value = evaluate_expression(right, scopes)?;
            evaluate_binary(*operator, left_value, right_value, *position)
        }
        Expression::LogicalAnd {
            left,
            right,
            position,
        } => {
            let left_value = evaluate_expression(left, scopes)?;
            let left_bool = match left_value {
                Value::Bool(value) => value,
                other => {
                    return Err(positioned_error(
                        format!(
                            "type mismatch in '&&': expected a boolean, found {}",
                            value_type_name(&other)
                        ),
                        *position,
                    ))
                }
            };
            if !left_bool {
                // Short-circuit: the right operand must not be evaluated.
                return Ok(Value::Bool(false));
            }
            match evaluate_expression(right, scopes)? {
                Value::Bool(value) => Ok(Value::Bool(value)),
                other => Err(positioned_error(
                    format!(
                        "type mismatch in '&&': expected a boolean, found {}",
                        value_type_name(&other)
                    ),
                    *position,
                )),
            }
        }
        Expression::LogicalOr {
            left,
            right,
            position,
        } => {
            let left_value = evaluate_expression(left, scopes)?;
            let left_bool = match left_value {
                Value::Bool(value) => value,
                other => {
                    return Err(positioned_error(
                        format!(
                            "type mismatch in '||': expected a boolean, found {}",
                            value_type_name(&other)
                        ),
                        *position,
                    ))
                }
            };
            if left_bool {
                // Short-circuit: the right operand must not be evaluated.
                return Ok(Value::Bool(true));
            }
            match evaluate_expression(right, scopes)? {
                Value::Bool(value) => Ok(Value::Bool(value)),
                other => Err(positioned_error(
                    format!(
                        "type mismatch in '||': expected a boolean, found {}",
                        value_type_name(&other)
                    ),
                    *position,
                )),
            }
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
            position,
        } => {
            let condition_value = evaluate_expression(condition, scopes)?;
            let condition_bool = match condition_value {
                Value::Bool(value) => value,
                other => {
                    return Err(positioned_error(
                        format!(
                            "if condition must be a boolean, found {}",
                            value_type_name(&other)
                        ),
                        *position,
                    ))
                }
            };

            if condition_bool {
                evaluate_block(then_branch, scopes)
            } else {
                evaluate_block(else_branch, scopes)
            }
        }
    }
}

fn evaluate_binary(
    operator: BinaryOperator,
    left: Value,
    right: Value,
    position: Option<SourcePosition>,
) -> Result<Value, Error> {
    match operator {
        BinaryOperator::Add => match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(b)
                .map(Value::Int)
                .ok_or_else(|| positioned_error("integer addition overflow", position)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
            (a, b) => Err(positioned_error(
                format!(
                    "type mismatch in '+': expected two integers or two strings, found {} and {}",
                    value_type_name(&a),
                    value_type_name(&b)
                ),
                position,
            )),
        },
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let symbol = if operator == BinaryOperator::Equal {
                "=="
            } else {
                "!="
            };
            let equal = match (left, right) {
                (Value::Int(a), Value::Int(b)) => a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (Value::String(a), Value::String(b)) => a == b,
                (a, b) => {
                    return Err(positioned_error(
                        format!(
                            "type mismatch in '{symbol}': expected two values of the same type, found {} and {}",
                            value_type_name(&a),
                            value_type_name(&b)
                        ),
                        position,
                    ))
                }
            };
            Ok(Value::Bool(if operator == BinaryOperator::Equal {
                equal
            } else {
                !equal
            }))
        }
        BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual => {
            let symbol = match operator {
                BinaryOperator::Subtract => "-",
                BinaryOperator::Multiply => "*",
                BinaryOperator::Divide => "/",
                BinaryOperator::Remainder => "%",
                BinaryOperator::LessThan => "<",
                BinaryOperator::LessThanOrEqual => "<=",
                BinaryOperator::GreaterThan => ">",
                BinaryOperator::GreaterThanOrEqual => ">=",
                _ => unreachable!("handled above"),
            };

            let (a, b) = match (left, right) {
                (Value::Int(a), Value::Int(b)) => (a, b),
                (a, b) => {
                    return Err(positioned_error(
                        format!(
                            "type mismatch in '{symbol}': expected two integers, found {} and {}",
                            value_type_name(&a),
                            value_type_name(&b)
                        ),
                        position,
                    ))
                }
            };

            match operator {
                BinaryOperator::Subtract => a
                    .checked_sub(b)
                    .map(Value::Int)
                    .ok_or_else(|| positioned_error("integer subtraction overflow", position)),
                BinaryOperator::Multiply => a
                    .checked_mul(b)
                    .map(Value::Int)
                    .ok_or_else(|| positioned_error("integer multiplication overflow", position)),
                BinaryOperator::Divide => {
                    if b == 0 {
                        Err(positioned_error("division by zero", position))
                    } else {
                        a.checked_div(b)
                            .map(Value::Int)
                            .ok_or_else(|| positioned_error("integer division overflow", position))
                    }
                }
                BinaryOperator::Remainder => {
                    if b == 0 {
                        Err(positioned_error("division by zero", position))
                    } else {
                        a.checked_rem(b)
                            .map(Value::Int)
                            .ok_or_else(|| positioned_error("integer remainder overflow", position))
                    }
                }
                BinaryOperator::LessThan => Ok(Value::Bool(a < b)),
                BinaryOperator::LessThanOrEqual => Ok(Value::Bool(a <= b)),
                BinaryOperator::GreaterThan => Ok(Value::Bool(a > b)),
                BinaryOperator::GreaterThanOrEqual => Ok(Value::Bool(a >= b)),
                _ => unreachable!("handled above"),
            }
        }
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Int(_) => "integer",
        Value::Bool(_) => "boolean",
        Value::String(_) => "string",
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
    use crate::typecheck;
    use crate::Value;

    fn evaluate_source(input: &str) -> Result<Value, Error> {
        let tokens = Lexer::new(input).tokenize()?;
        let program = Parser::new(&tokens).parse()?;
        typecheck::check(&program)?;
        evaluate(&program)
    }

    fn int(value: i64) -> Value {
        Value::Int(value)
    }

    fn boolean(value: bool) -> Value {
        Value::Bool(value)
    }

    fn string(value: &str) -> Value {
        Value::String(value.to_owned())
    }

    fn error_message(input: &str) -> String {
        evaluate_source(input).unwrap_err().to_string()
    }

    #[test]
    fn respects_operator_precedence() {
        assert_eq!(evaluate_source("1 + 2 * 3"), Ok(int(7)));
        assert_eq!(evaluate_source("(1 + 2) * 3"), Ok(int(9)));
        assert_eq!(evaluate_source("-2 * 3 + 4"), Ok(int(-2)));
    }

    #[test]
    fn preserves_associativity_and_whitespace_behavior() {
        assert_eq!(evaluate_source("10 - 3 - 2"), Ok(int(5)));
        assert_eq!(evaluate_source("20 / 5 / 2"), Ok(int(2)));
        assert_eq!(evaluate_source(" \t12\n / 5 "), Ok(int(2)));
    }

    #[test]
    fn evaluates_signed_values_and_checked_arithmetic() {
        assert_eq!(evaluate_source("1 - 3 * 2"), Ok(int(-5)));
        assert_eq!(evaluate_source("0 - 7 / 3"), Ok(int(-2)));
        assert_eq!(evaluate_source("-1"), Ok(int(-1)));
        assert_eq!(evaluate_source("-(1 + 2)"), Ok(int(-3)));
        assert_eq!(evaluate_source("3 * -2"), Ok(int(-6)));
        assert_eq!(evaluate_source("-2 * -3"), Ok(int(6)));
        assert_eq!(evaluate_source("--1"), Ok(int(1)));
        assert_eq!(evaluate_source("---1"), Ok(int(-1)));
    }

    #[test]
    fn evaluates_comparisons_as_booleans() {
        assert_eq!(evaluate_source("1 < 2"), Ok(boolean(true)));
        assert_eq!(evaluate_source("2 < 1"), Ok(boolean(false)));
        assert_eq!(evaluate_source("2 <= 2"), Ok(boolean(true)));
        assert_eq!(evaluate_source("3 <= 2"), Ok(boolean(false)));
        assert_eq!(evaluate_source("2 > 1"), Ok(boolean(true)));
        assert_eq!(evaluate_source("1 > 2"), Ok(boolean(false)));
        assert_eq!(evaluate_source("2 >= 2"), Ok(boolean(true)));
        assert_eq!(evaluate_source("1 >= 2"), Ok(boolean(false)));
        assert_eq!(evaluate_source("2 == 2"), Ok(boolean(true)));
        assert_eq!(evaluate_source("2 == 3"), Ok(boolean(false)));
        assert_eq!(evaluate_source("2 != 3"), Ok(boolean(true)));
        assert_eq!(evaluate_source("2 != 2"), Ok(boolean(false)));
        assert_eq!(evaluate_source("1 + 2 < 2 * 2"), Ok(boolean(true)));
        assert_eq!(evaluate_source("10 - 3 >= 2 + 6"), Ok(boolean(false)));
        assert_eq!(evaluate_source("(1 < 2) == (2 < 3)"), Ok(boolean(true)));
        assert_eq!(evaluate_source("(1 < 2) == (2 > 3)"), Ok(boolean(false)));
    }

    #[test]
    fn evaluates_boolean_literals_and_logical_operators() {
        assert_eq!(evaluate_source("true"), Ok(boolean(true)));
        assert_eq!(evaluate_source("false"), Ok(boolean(false)));
        assert_eq!(evaluate_source("!true"), Ok(boolean(false)));
        assert_eq!(evaluate_source("!!true"), Ok(boolean(true)));
        assert_eq!(evaluate_source("true && false"), Ok(boolean(false)));
        assert_eq!(evaluate_source("true && true"), Ok(boolean(true)));
        assert_eq!(evaluate_source("true || false"), Ok(boolean(true)));
        assert_eq!(evaluate_source("false || false"), Ok(boolean(false)));
        // && binds tighter than ||.
        assert_eq!(evaluate_source("true || false && false"), Ok(boolean(true)));
        assert_eq!(evaluate_source("1 < 2 && 3 < 4"), Ok(boolean(true)));
        assert_eq!(evaluate_source("1 < 2 && 4 < 3"), Ok(boolean(false)));
    }

    #[test]
    fn logical_operators_short_circuit() {
        // The right side contains a division by zero that must never run.
        assert_eq!(
            evaluate_source("false && 8 / (3 - 3) == 1"),
            Ok(boolean(false))
        );
        assert_eq!(
            evaluate_source("true || 8 / (3 - 3) == 1"),
            Ok(boolean(true))
        );
        // The non-short-circuiting side still fails normally.
        assert_eq!(
            error_message("true && 8 / (3 - 3) == 1"),
            "division by zero"
        );
        assert_eq!(
            error_message("false || 8 / (3 - 3) == 1"),
            "division by zero"
        );
    }

    #[test]
    fn evaluates_strings_and_concatenation() {
        assert_eq!(evaluate_source("\"hello\""), Ok(string("hello")));
        assert_eq!(
            evaluate_source("\"hello\" + \" \" + \"world\""),
            Ok(string("hello world"))
        );
        assert_eq!(
            evaluate_source("let s = \"a\" + \"b\"; s + \"c\""),
            Ok(string("abc"))
        );
        assert_eq!(
            evaluate_source(r#""line\nbreak\tend""#),
            Ok(string("line\nbreak\tend"))
        );
        assert_eq!(
            evaluate_source(r#""say \"hi\" and \\done""#),
            Ok(string("say \"hi\" and \\done"))
        );
    }

    #[test]
    fn evaluates_string_equality() {
        assert_eq!(evaluate_source("\"a\" == \"a\""), Ok(boolean(true)));
        assert_eq!(evaluate_source("\"a\" == \"b\""), Ok(boolean(false)));
        assert_eq!(evaluate_source("\"a\" != \"b\""), Ok(boolean(true)));
        assert_eq!(
            evaluate_source("let s = \"hi\"; s == \"hi\""),
            Ok(boolean(true))
        );
        assert_eq!(evaluate_source("true == true"), Ok(boolean(true)));
        assert_eq!(evaluate_source("true != false"), Ok(boolean(true)));
    }

    #[test]
    fn accepts_the_signed_minimum_literal() {
        assert_eq!(evaluate_source("-9223372036854775808"), Ok(int(i64::MIN)));
        assert_eq!(evaluate_source("-(9223372036854775808)"), Ok(int(i64::MIN)));
        assert_eq!(
            evaluate_source("0 - 9223372036854775807 - 1 / (0 - 1)"),
            Ok(int(-9223372036854775806))
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
    fn reports_type_errors_before_evaluation() {
        assert_eq!(
            evaluate_source("8 / (3 - 3) + true")
                .unwrap_err()
                .to_string(),
            "type mismatch in '+': expected two integers or two strings, found integer and boolean"
        );
    }

    #[test]
    fn evaluator_defensively_rejects_ill_typed_programs_without_typechecking() {
        // The normal pipeline type-checks before evaluating, but the evaluator
        // must still report a clear error instead of panicking on any program
        // the parser accepts (the fuzz targets exercise exactly this). Bypass
        // the type checker here to cover those defensive paths.
        let parse_only = |source: &str| {
            let tokens = Lexer::new(source).tokenize().unwrap();
            Parser::new(&tokens).parse().unwrap()
        };

        for source in [
            "1 + true",
            "true - 1",
            "true * 1",
            "\"a\" * 2",
            "8 / true",
            "1 % \"a\"",
            "true < 1",
            "1 <= true",
            "true > 1",
            "1 >= true",
            "1 == true",
            "true != 1",
            "-true",
            "!5",
            "1 && true",
            "true && 1",
            "1 || true",
            // `true || 1` would short-circuit without evaluating the
            // ill-typed right operand, so use a false left side instead.
            "false || 1",
            "if 1 { 2 } else { 3 }",
        ] {
            let parsed = parse_only(source);
            let error = evaluate(&parsed).unwrap_err();
            let message = error.to_string();
            assert!(
                message.starts_with("type mismatch") || message.starts_with("if condition"),
                "source: {source}, message: {message}"
            );
        }
    }

    #[test]
    fn evaluates_modulo_with_truncating_division_semantics() {
        assert_eq!(evaluate_source("10 % 3"), Ok(int(1)));
        assert_eq!(evaluate_source("20 % 5"), Ok(int(0)));
        assert_eq!(evaluate_source("-7 % 3"), Ok(int(-1)));
        assert_eq!(evaluate_source("7 % -3"), Ok(int(1)));
        assert_eq!(evaluate_source("-7 % -3"), Ok(int(-1)));
    }

    #[test]
    fn evaluates_modulo_at_multiplicative_precedence() {
        assert_eq!(evaluate_source("2 + 3 * 4 % 5"), Ok(int(4)));
        assert_eq!(evaluate_source("(2 + 3) % 4"), Ok(int(1)));
        assert_eq!(evaluate_source("17 % 5 * 2"), Ok(int(4)));
        assert_eq!(evaluate_source("20 % 6 % 5"), Ok(int(2)));
    }

    #[test]
    fn evaluates_modulo_in_variable_declarations() {
        assert_eq!(
            evaluate_source("let a = 10; let b = a % 3; b + 1"),
            Ok(int(2))
        );
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
            Some(crate::error::SourcePosition { line: 1, column: 3 })
        );
    }

    #[test]
    fn evaluates_immutable_variables_in_order() {
        assert_eq!(
            evaluate_source("let rate = 20; let quantity = 5; rate * quantity"),
            Ok(int(100))
        );
        assert_eq!(
            evaluate_source("let first = 2; let second = first + 3; second * 4"),
            Ok(int(20))
        );
        assert_eq!(evaluate_source("let _value2 = 7; _value2"), Ok(int(7)));
    }

    #[test]
    fn evaluates_if_expressions() {
        assert_eq!(evaluate_source("if true { 10 } else { 20 }"), Ok(int(10)));
        assert_eq!(evaluate_source("if false { 10 } else { 20 }"), Ok(int(20)));
        assert_eq!(
            evaluate_source("if 1 < 2 { \"a\" } else { \"b\" }"),
            Ok(string("a"))
        );
        assert_eq!(
            evaluate_source("if true { true } else { false }"),
            Ok(boolean(true))
        );
        assert_eq!(
            evaluate_source("if true { 1 } else { 2 } + 100"),
            Ok(int(101))
        );
    }

    #[test]
    fn evaluates_nested_if_expressions() {
        assert_eq!(
            evaluate_source("if (if true { false } else { true }) { 1 } else { 2 }"),
            Ok(int(2))
        );
        assert_eq!(
            evaluate_source("if 5 > 3 { if 2 > 1 { 1 } else { 2 } } else { 3 }"),
            Ok(int(1))
        );
    }

    #[test]
    fn evaluates_blocks_with_scoped_declarations() {
        assert_eq!(
            evaluate_source("if true { let bonus = 3; 10 + bonus } else { 10 }"),
            Ok(int(13))
        );
        assert_eq!(
            evaluate_source("if true { let x = 1; let y = 2; x + y } else { 0 }"),
            Ok(int(3))
        );
    }

    #[test]
    fn blocks_shadow_outer_names_without_leaking() {
        assert_eq!(
            evaluate_source("let x = 1; if true { let x = 2; x } else { x }"),
            Ok(int(2))
        );
        assert_eq!(
            evaluate_source("let x = 1; if false { let x = 2; x } else { x }"),
            Ok(int(1))
        );
        assert_eq!(
            evaluate_source("let s = \"outer\"; if true { let s = \"inner\"; s } else { s }"),
            Ok(string("inner"))
        );
        // The shadowed declaration does not escape the block: the final `+ x`
        // still sees the outer x = 1, so the result is 3, not 4.
        assert_eq!(
            evaluate_source("let x = 1; (if true { let x = 2; x } else { x }) + x"),
            Ok(int(3))
        );
    }

    #[test]
    fn declarations_can_use_if_expressions() {
        assert_eq!(
            evaluate_source("let x = if true { 1 } else { 2 }; x * 3"),
            Ok(int(3))
        );
        assert_eq!(
            evaluate_source(
                "let temperature = 32; let verdict = if temperature > 30 { \"hot\" } else { \"cold\" }; verdict"
            ),
            Ok(string("hot"))
        );
    }
}
