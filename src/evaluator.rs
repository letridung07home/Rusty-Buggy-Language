use std::collections::HashMap;

use crate::ast::{BinaryOperator, Block, Declaration, Expression, Program};
use crate::error::{Error, SourcePosition};
use crate::typecheck::ResolvedFunctions;
use crate::Value;

/// The maximum depth of nested function calls the evaluator allows before
/// reporting a clear error. Kept low enough that even a debug build's default
/// thread stack (and the MSRV test harness) cannot be overflowed by a few
/// hundred nested calls, mirroring the parser's nesting-depth limit and the
/// language's "clear error instead of crash" hardening posture.
const MAX_CALL_DEPTH: usize = 128;

/// The maximum depth of nested expression evaluation the evaluator allows before
/// reporting a clear error. Each `evaluate_expression` level costs an AST-descent
/// frame on top of the call-depth guard above, so a pathological expression
/// (issue #13: a 44-deep unary-minus chain inside a recursive function body) can
/// exhaust the stack long before `MAX_CALL_DEPTH` trips. 2048 levels at a
/// worst-case ~2 KB sanitizer-inflated frame is ~4 MB, half of an 8 MB stack,
/// while still admitting roughly twice the deepest legitimate program (a
/// 128-call factorial recursion costs ~1.2k frames). The reported message
/// mirrors the parser's nesting-depth limit.
const MAX_EVAL_DEPTH: usize = 2048;

pub(crate) fn evaluate(program: &Program, functions: &ResolvedFunctions) -> Result<Value, Error> {
    let mut scopes = vec![HashMap::new()];
    let mut call_depth = 0_usize;
    let mut eval_depth = 0_usize;
    evaluate_body(
        &program.declarations,
        &program.expression,
        functions,
        &mut scopes,
        &mut call_depth,
        &mut eval_depth,
    )
}

/// Evaluates declarations into the current innermost scope and then the final
/// expression, mirroring the type checker's scope discipline.
fn evaluate_body(
    declarations: &[Declaration],
    expression: &Expression,
    functions: &ResolvedFunctions,
    scopes: &mut Vec<HashMap<String, Value>>,
    call_depth: &mut usize,
    eval_depth: &mut usize,
) -> Result<Value, Error> {
    for declaration in declarations {
        let value = evaluate_expression(
            &declaration.initializer,
            functions,
            scopes,
            call_depth,
            eval_depth,
        )?;
        let current = scopes.last_mut().expect("the scope stack is never empty");
        current.insert(declaration.name.clone(), value);
    }
    evaluate_expression(expression, functions, scopes, call_depth, eval_depth)
}

fn evaluate_block(
    block: &Block,
    functions: &ResolvedFunctions,
    scopes: &mut Vec<HashMap<String, Value>>,
    call_depth: &mut usize,
    eval_depth: &mut usize,
) -> Result<Value, Error> {
    scopes.push(HashMap::new());
    let result = evaluate_body(
        &block.declarations,
        &block.expression,
        functions,
        scopes,
        call_depth,
        eval_depth,
    );
    scopes.pop();
    result
}

fn evaluate_expression(
    expression: &Expression,
    functions: &ResolvedFunctions,
    scopes: &mut Vec<HashMap<String, Value>>,
    call_depth: &mut usize,
    eval_depth: &mut usize,
) -> Result<Value, Error> {
    *eval_depth += 1;
    if *eval_depth > MAX_EVAL_DEPTH {
        *eval_depth -= 1;
        return Err(positioned_error(
            "program too deeply nested",
            expression.position(),
        ));
    }

    let result = match expression {
        Expression::Literal { value, position } => i64::try_from(*value)
            .map(Value::Int)
            .map_err(|_| positioned_error("integer literal out of range", *position)),
        Expression::StringLiteral { value, .. } => Ok(Value::String(value.clone())),
        Expression::BoolLiteral { value, .. } => Ok(Value::Bool(*value)),
        Expression::Variable { name, position } => {
            match scopes.iter().rev().find_map(|s| s.get(name).cloned()) {
                Some(value) => Ok(value),
                None => Err(positioned_error(
                    format!("undefined variable: '{name}'"),
                    *position,
                )),
            }
        }
        Expression::Call {
            callee,
            arguments,
            position,
        } => evaluate_call(
            callee, arguments, *position, functions, scopes, call_depth, eval_depth,
        ),
        Expression::UnaryNegation { operand, position } => {
            // The magnitude 2^63 has no unnegated representation; a literal of
            // that magnitude is only valid under an immediately applied `-`.
            let is_i64_min = matches!(
                operand.as_ref(),
                Expression::Literal { value, .. } if *value == (i64::MAX as u64) + 1
            );
            if is_i64_min {
                Ok(Value::Int(i64::MIN))
            } else {
                match evaluate_expression(operand, functions, scopes, call_depth, eval_depth)? {
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
        }
        Expression::UnaryNot { operand, position } => {
            match evaluate_expression(operand, functions, scopes, call_depth, eval_depth)? {
                Value::Bool(value) => Ok(Value::Bool(!value)),
                other => Err(positioned_error(
                    format!(
                        "type mismatch in '!': expected a boolean, found {}",
                        value_type_name(&other)
                    ),
                    *position,
                )),
            }
        }
        Expression::Binary {
            operator,
            left,
            right,
            position,
        } => {
            let left_value = evaluate_expression(left, functions, scopes, call_depth, eval_depth)?;
            let right_value =
                evaluate_expression(right, functions, scopes, call_depth, eval_depth)?;
            evaluate_binary(*operator, left_value, right_value, *position)
        }
        Expression::LogicalAnd {
            left,
            right,
            position,
        } => {
            let left_value = evaluate_expression(left, functions, scopes, call_depth, eval_depth)?;
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
                Ok(Value::Bool(false))
            } else {
                match evaluate_expression(right, functions, scopes, call_depth, eval_depth)? {
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
        }
        Expression::LogicalOr {
            left,
            right,
            position,
        } => {
            let left_value = evaluate_expression(left, functions, scopes, call_depth, eval_depth)?;
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
                Ok(Value::Bool(true))
            } else {
                match evaluate_expression(right, functions, scopes, call_depth, eval_depth)? {
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
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
            position,
        } => {
            let condition_value =
                evaluate_expression(condition, functions, scopes, call_depth, eval_depth)?;
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
                evaluate_block(then_branch, functions, scopes, call_depth, eval_depth)
            } else {
                evaluate_block(else_branch, functions, scopes, call_depth, eval_depth)
            }
        }
    };

    *eval_depth -= 1;
    result
}

/// Evaluates a function call: resolves the callee, evaluates the arguments,
/// binds the parameters into a fresh scope, and evaluates the body. Recursion is
/// natural because each call looks its callee up again in the environment. The
/// call depth is guarded so runaway recursion reports a clear error. Builtin
/// calls are dispatched through the type checker's signature table first (the
/// two never share a name) and consume no call depth.
fn evaluate_call(
    callee: &str,
    arguments: &[Expression],
    position: Option<SourcePosition>,
    functions: &ResolvedFunctions,
    scopes: &mut Vec<HashMap<String, Value>>,
    call_depth: &mut usize,
    eval_depth: &mut usize,
) -> Result<Value, Error> {
    if let Some(builtin) = crate::typecheck::builtin_signature(callee) {
        return evaluate_builtin_call(
            builtin, arguments, position, functions, scopes, call_depth, eval_depth,
        );
    }

    let function = functions
        .get(callee)
        .ok_or_else(|| positioned_error(format!("undefined function: '{callee}'"), position))?;

    if arguments.len() != function.parameters.len() {
        return Err(positioned_error(
            format!(
                "wrong number of arguments for function '{callee}': expected {}, found {}",
                function.parameters.len(),
                arguments.len()
            ),
            position,
        ));
    }

    let mut argument_values = Vec::with_capacity(arguments.len());
    for argument in arguments {
        argument_values.push(evaluate_expression(
            argument, functions, scopes, call_depth, eval_depth,
        )?);
    }

    // The type checker enforces argument types, but the evaluator defends the
    // same invariant in case a fuzz target drives it without type-checking.
    for (index, value) in argument_values.iter().enumerate() {
        let expected = function.parameter_types[index];
        if value_type_of(value) != expected {
            return Err(positioned_error(
                format!(
                    "type mismatch in call to '{callee}': expected an argument of type {}, found {}",
                    expected.name(),
                    value_type_name(value)
                ),
                position,
            ));
        }
    }

    *call_depth += 1;
    if *call_depth > MAX_CALL_DEPTH {
        *call_depth -= 1;
        return Err(positioned_error("call depth limit exceeded", position));
    }

    scopes.push(HashMap::new());
    {
        let current = scopes.last_mut().expect("the scope stack is never empty");
        for (name, value) in function.parameters.iter().zip(argument_values.iter()) {
            current.insert(name.clone(), value.clone());
        }
    }
    let result = evaluate_body(
        &function.body.declarations,
        &function.body.expression,
        functions,
        scopes,
        call_depth,
        eval_depth,
    );
    scopes.pop();
    *call_depth -= 1;

    result
}

/// Evaluates a call to a fixed-signature builtin function (`len`,
/// `int_to_string`, `string_to_int`, `bool_to_int`, `int_to_bool`), mirroring
/// the user-function runtime defenses with the builtin's name substituted. The
/// call itself adds no evaluation frame, so it consumes no call depth.
fn evaluate_builtin_call(
    builtin: &crate::typecheck::Builtin,
    arguments: &[Expression],
    position: Option<SourcePosition>,
    functions: &ResolvedFunctions,
    scopes: &mut Vec<HashMap<String, Value>>,
    call_depth: &mut usize,
    eval_depth: &mut usize,
) -> Result<Value, Error> {
    let name = builtin.name;
    if arguments.len() != builtin.parameter_types.len() {
        return Err(positioned_error(
            format!(
                "wrong number of arguments for function '{name}': expected {}, found {}",
                builtin.parameter_types.len(),
                arguments.len()
            ),
            position,
        ));
    }

    let mut argument_values = Vec::with_capacity(arguments.len());
    for argument in arguments {
        argument_values.push(evaluate_expression(
            argument, functions, scopes, call_depth, eval_depth,
        )?);
    }

    // The type checker enforces builtin argument types, but the evaluator
    // defends the same invariant in case a fuzz target drives it without
    // type-checking.
    for (index, value) in argument_values.iter().enumerate() {
        let expected = builtin.parameter_types[index];
        if value_type_of(value) != expected {
            return Err(positioned_error(
                format!(
                    "type mismatch in call to '{name}': expected an argument of type {}, found {}",
                    expected.name(),
                    value_type_name(value)
                ),
                position,
            ));
        }
    }

    Ok(match (name, argument_values.as_slice()) {
        ("len", [Value::String(text)]) => Value::Int(text.chars().count() as i64),
        ("int_to_string", [Value::Int(value)]) => Value::String(value.to_string()),
        ("string_to_int", [Value::String(text)]) => match parse_builtin_integer(text) {
            Some(value) => Value::Int(value),
            None => {
                return Err(positioned_error(
                    format!("invalid integer text: '{text}'"),
                    position,
                ))
            }
        },
        ("bool_to_int", [Value::Bool(flag)]) => Value::Int(i64::from(*flag)),
        ("int_to_bool", [Value::Int(value)]) => Value::Bool(*value != 0),
        _ => unreachable!("the signature defenses above cover every builtin"),
    })
}

/// Parses the text argument of `string_to_int`: an optional leading `-`
/// followed by one or more ASCII digits, with no whitespace or `+`, that must
/// fit into an i64. Anything else (empty text, a lone `-`, surrounding
/// whitespace, non-ASCII digits, an out-of-range magnitude) is rejected so the
/// caller can report `invalid integer text`.
fn parse_builtin_integer(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // With the digits-only shape enforced above, overflow is the only way the
    // checked parse can fail.
    text.parse::<i64>().ok()
}

fn value_type_of(value: &Value) -> crate::ast::Type {
    match value {
        Value::Int(_) => crate::ast::Type::Int,
        Value::Bool(_) => crate::ast::Type::Bool,
        Value::String(_) => crate::ast::Type::String,
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
        BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual => {
            let symbol = match operator {
                BinaryOperator::LessThan => "<",
                BinaryOperator::LessThanOrEqual => "<=",
                BinaryOperator::GreaterThan => ">",
                BinaryOperator::GreaterThanOrEqual => ">=",
                _ => unreachable!("handled above"),
            };

            // Ordering compares two integers, or two strings lexicographically
            // (str Ord). Any other pairing keeps the integer-path defense and
            // its existing message.
            let ordering = match (left, right) {
                (Value::Int(a), Value::Int(b)) => a.cmp(&b),
                (Value::String(a), Value::String(b)) => a.cmp(&b),
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

            Ok(Value::Bool(match operator {
                BinaryOperator::LessThan => ordering.is_lt(),
                BinaryOperator::LessThanOrEqual => ordering.is_le(),
                BinaryOperator::GreaterThan => ordering.is_gt(),
                BinaryOperator::GreaterThanOrEqual => ordering.is_ge(),
                _ => unreachable!("handled above"),
            }))
        }
        BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder => {
            let symbol = match operator {
                BinaryOperator::Subtract => "-",
                BinaryOperator::Multiply => "*",
                BinaryOperator::Divide => "/",
                BinaryOperator::Remainder => "%",
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
        let functions = typecheck::resolve(&program)?;
        evaluate(&program, &functions)
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

    // Debug test threads default to a small stack; the guard legitimately allows ~2048 frames.
    fn error_message_on_large_stack(source: String) -> String {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(move || error_message(&source))
            .expect("test thread spawned")
            .join()
            .expect("test thread did not panic")
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
        let empty = crate::typecheck::ResolvedFunctions::default();

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
            let error = evaluate(&parsed, &empty).unwrap_err();
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

    // --- Functions ---

    #[test]
    fn evaluates_simple_function_calls() {
        assert_eq!(evaluate_source("fn sq(x) = { x * x }; sq(5)"), Ok(int(25)));
        assert_eq!(
            evaluate_source("fn inc(x) = { x + 1 }; inc(41)"),
            Ok(int(42))
        );
    }

    #[test]
    fn evaluates_multiple_parameters_and_blocks_with_locals() {
        assert_eq!(
            evaluate_source(
                "fn max(a, b) = { let big = if a > b { a } else { b }; big }; max(3, 7)"
            ),
            Ok(int(7))
        );
        assert_eq!(
            evaluate_source("fn hypo(a) = { let ds = a * 2; ds + 1 }; hypo(10)"),
            Ok(int(21))
        );
    }

    #[test]
    fn functions_can_refer_to_other_functions() {
        assert_eq!(
            evaluate_source(
                "fn double(x) = { x + x }; fn quad(x) = { double(double(x)) }; quad(3)"
            ),
            Ok(int(12))
        );
    }

    #[test]
    fn evaluates_recursion() {
        assert_eq!(
            evaluate_source("fn fact(n) = { if n <= 1 { 1 } else { n * fact(n - 1) } }; fact(5)"),
            Ok(int(120))
        );
        assert_eq!(
            evaluate_source(
                "fn fib(n) = { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)"
            ),
            Ok(int(55))
        );
    }

    #[test]
    fn evaluates_mutual_recursion() {
        assert_eq!(
            evaluate_source(
                "fn even(n) = { if n == 0 { true } else { odd(n - 1) } }; fn odd(n) = { if n == 0 { false } else { even(n - 1) } }; even(10)"
            ),
            Ok(boolean(true))
        );
        assert_eq!(
            evaluate_source(
                "fn even(n) = { if n == 0 { true } else { odd(n - 1) } }; fn odd(n) = { if n == 0 { false } else { even(n - 1) } }; odd(10)"
            ),
            Ok(boolean(false))
        );
    }

    #[test]
    fn parameters_are_scoped_to_the_body() {
        // A parameter must not leak into the top-level final expression.
        assert_eq!(
            error_message("fn f(x) = { x + 1 }; f(1) + x"),
            "undefined variable: 'x'"
        );
    }

    #[test]
    fn infinite_recursion_reports_a_clear_error() {
        // A counting function that recurses deeper than the call-depth guard,
        // so the guard (not the terminal case) must fire. The program still
        // type-checks: `count(n)` is an integer.
        let source = "fn count(n) = { if n == 0 { n } else { count(n - 1) } }; count(2000)";
        assert_eq!(error_message(source), "call depth limit exceeded");
    }

    #[test]
    fn deep_body_recursion_hits_the_evaluation_depth_guard() {
        // Issue #13 class: a recursive body whose expression nesting is deep
        // costs call-depth x body-depth evaluator frames, so without the
        // evaluation-depth guard this shape burned call-depth x body-depth
        // stack frames on its way to reporting `call depth limit exceeded`.
        // The 100 unary minuses are parser-legal (the parser's nesting limit
        // is 128), so the evaluation-depth guard must be the one to fire, with
        // the parser's nesting message.
        let mut source = String::from("fn s()={");
        source.push_str(&"-".repeat(100));
        source.push_str("s()-8};s()-7");
        assert_eq!(
            error_message_on_large_stack(source),
            "program too deeply nested"
        );
    }

    #[test]
    fn issue_13_fuzz_crash_input_reports_a_clean_error() {
        // The exact 64-byte program from the nightly ASan fuzz run reported in
        // GitHub issue #13: `fn`, newline, `s()={`, 44 minus signs,
        // `s()-8};s()-7`. Before the evaluation-depth guard this overflowed
        // the stack under ASan; the guard now trips at ~44 calls, well before
        // the 128-call limit and before any stack exhaustion.
        let source = "fn\ns()={--------------------------------------------s()-8};s()-7".to_owned();
        assert_eq!(
            error_message_on_large_stack(source),
            "program too deeply nested"
        );
    }

    #[test]
    fn deep_if_block_recursion_hits_the_evaluation_depth_guard() {
        // The issue #13 shape again, but with the recursion hidden inside an
        // if/else block: the branch descent adds evaluator frames on top of
        // the unary-minus chain, so the evaluation-depth guard must still fire
        // with the same clean nesting error.
        let mut source = String::from("fn s()={ if true { ");
        source.push_str(&"-".repeat(100));
        source.push_str("s()-8 } else { 0 } }; s()");
        assert_eq!(
            error_message_on_large_stack(source),
            "program too deeply nested"
        );
    }

    // --- Stdlib builtins and string ordering ---

    #[test]
    fn evaluates_the_len_builtin_over_string_characters() {
        assert_eq!(evaluate_source("len(\"\")"), Ok(int(0)));
        assert_eq!(evaluate_source("len(\"hello\")"), Ok(int(5)));
        // len counts characters, not bytes: "héllo" is 5 chars over 6 bytes.
        assert_eq!(evaluate_source("len(\"héllo\")"), Ok(int(5)));
    }

    #[test]
    fn evaluates_the_int_to_string_builtin() {
        assert_eq!(evaluate_source("int_to_string(0)"), Ok(string("0")));
        assert_eq!(evaluate_source("int_to_string(42)"), Ok(string("42")));
        assert_eq!(evaluate_source("int_to_string(-5)"), Ok(string("-5")));
    }

    #[test]
    fn evaluates_the_string_to_int_builtin() {
        assert_eq!(evaluate_source("string_to_int(\"0\")"), Ok(int(0)));
        assert_eq!(evaluate_source("string_to_int(\"-7\")"), Ok(int(-7)));
        // Leading zeros are accepted and normalized.
        assert_eq!(evaluate_source("string_to_int(\"007\")"), Ok(int(7)));
        // The signed minimum fits even though its magnitude does not.
        assert_eq!(
            evaluate_source("string_to_int(\"-9223372036854775808\")"),
            Ok(int(i64::MIN))
        );
    }

    #[test]
    fn string_to_int_rejects_invalid_text() {
        for text in [
            "",
            "-",
            "+5",
            " 5",
            "5 ",
            "12.5",
            "abc",
            "9223372036854775808",
        ] {
            let source = format!("string_to_int(\"{text}\")");
            assert_eq!(
                error_message(&source),
                format!("invalid integer text: '{text}'"),
                "source: {source}"
            );
        }
    }

    #[test]
    fn evaluates_the_bool_and_int_conversion_builtins() {
        assert_eq!(evaluate_source("bool_to_int(true)"), Ok(int(1)));
        assert_eq!(evaluate_source("bool_to_int(false)"), Ok(int(0)));
        assert_eq!(evaluate_source("int_to_bool(0)"), Ok(boolean(false)));
        assert_eq!(evaluate_source("int_to_bool(1)"), Ok(boolean(true)));
        // Any nonzero integer converts to true.
        assert_eq!(evaluate_source("int_to_bool(-2)"), Ok(boolean(true)));
    }

    #[test]
    fn evaluates_string_ordering_comparisons() {
        assert_eq!(evaluate_source("\"abc\" < \"abd\""), Ok(boolean(true)));
        assert_eq!(evaluate_source("\"b\" > \"a\""), Ok(boolean(true)));
        // Equal strings compare false under strict ordering.
        assert_eq!(evaluate_source("\"a\" < \"a\""), Ok(boolean(false)));
        assert_eq!(evaluate_source("\"a\" <= \"a\""), Ok(boolean(true)));
        assert_eq!(evaluate_source("\"abd\" >= \"abc\""), Ok(boolean(true)));
    }

    #[test]
    fn round_trips_integers_through_the_string_builtins() {
        assert_eq!(
            evaluate_source("string_to_int(int_to_string(-42))"),
            Ok(int(-42))
        );
    }
}
