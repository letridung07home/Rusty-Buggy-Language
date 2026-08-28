//! A lightweight static type checker.
//!
//! The checker walks the parsed program once and rejects ill-typed programs
//! with a positioned error before evaluation. It maintains a stack of lexical
//! scopes (the program top level plus one scope per `if`/`else` block) and
//! assigns each expression a [`Type`]. The parser has already rejected
//! duplicate declarations within a single scope, so re-declaring a name simply
//! overwrites its type in the current scope.

use std::collections::HashMap;

use crate::ast::{BinaryOperator, Declaration, Expression, Program};
use crate::error::{Error, SourcePosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    Int,
    Bool,
    String,
}

impl Type {
    fn name(self) -> &'static str {
        match self {
            Type::Int => "integer",
            Type::Bool => "boolean",
            Type::String => "string",
        }
    }
}

/// Type-checks a parsed program, returning the first positioned type error.
pub(crate) fn check(program: &Program) -> Result<(), Error> {
    let mut scopes = vec![HashMap::new()];
    check_body(&program.declarations, &program.expression, &mut scopes)?;
    Ok(())
}

/// Checks a list of declarations followed by a final expression against the
/// current innermost scope. Declarations bind in that scope; expressions may
/// reference any enclosing scope.
fn check_body(
    declarations: &[Declaration],
    expression: &Expression,
    scopes: &mut Vec<HashMap<String, Type>>,
) -> Result<Type, Error> {
    for declaration in declarations {
        let initializer_type = check_expression(&declaration.initializer, scopes)?;
        let current = scopes.last_mut().expect("the scope stack is never empty");
        current.insert(declaration.name.clone(), initializer_type);
    }
    check_expression(expression, scopes)
}

fn check_expression(
    expression: &Expression,
    scopes: &mut Vec<HashMap<String, Type>>,
) -> Result<Type, Error> {
    match expression {
        Expression::Literal { .. } => Ok(Type::Int),
        Expression::StringLiteral { .. } => Ok(Type::String),
        Expression::BoolLiteral { .. } => Ok(Type::Bool),
        Expression::Variable { name, position } => {
            for scope in scopes.iter().rev() {
                if let Some(found) = scope.get(name) {
                    return Ok(*found);
                }
            }
            Err(positioned(
                format!("undefined variable: '{name}'"),
                *position,
            ))
        }
        Expression::UnaryNegation { operand, position } => {
            let operand_type = check_expression(operand, scopes)?;
            if operand_type != Type::Int {
                return Err(positioned(
                    format!(
                        "type mismatch in '-': expected an integer, found {}",
                        operand_type.name()
                    ),
                    *position,
                ));
            }
            Ok(Type::Int)
        }
        Expression::UnaryNot { operand, position } => {
            let operand_type = check_expression(operand, scopes)?;
            if operand_type != Type::Bool {
                return Err(positioned(
                    format!(
                        "type mismatch in '!': expected a boolean, found {}",
                        operand_type.name()
                    ),
                    *position,
                ));
            }
            Ok(Type::Bool)
        }
        Expression::Binary {
            operator,
            left,
            right,
            position,
        } => {
            let left_type = check_expression(left, scopes)?;
            let right_type = check_expression(right, scopes)?;
            check_binary(*operator, left_type, right_type, *position)
        }
        Expression::LogicalAnd {
            left,
            right,
            position,
        } => check_logical("&&", left, right, *position, scopes),
        Expression::LogicalOr {
            left,
            right,
            position,
        } => check_logical("||", left, right, *position, scopes),
        Expression::If {
            condition,
            then_branch,
            else_branch,
            position,
        } => {
            let condition_type = check_expression(condition, scopes)?;
            if condition_type != Type::Bool {
                return Err(positioned(
                    format!(
                        "if condition must be a boolean, found {}",
                        condition_type.name()
                    ),
                    *position,
                ));
            }

            scopes.push(HashMap::new());
            let then_type = check_body(&then_branch.declarations, &then_branch.expression, scopes)?;
            scopes.pop();

            scopes.push(HashMap::new());
            let else_type = check_body(&else_branch.declarations, &else_branch.expression, scopes)?;
            scopes.pop();

            if then_type != else_type {
                return Err(positioned(
                    format!(
                        "if branches must have the same type, found {} and {}",
                        then_type.name(),
                        else_type.name()
                    ),
                    *position,
                ));
            }

            Ok(then_type)
        }
    }
}

fn check_logical(
    operator: &str,
    left: &Expression,
    right: &Expression,
    position: Option<SourcePosition>,
    scopes: &mut Vec<HashMap<String, Type>>,
) -> Result<Type, Error> {
    let left_type = check_expression(left, scopes)?;
    let right_type = check_expression(right, scopes)?;

    if left_type != Type::Bool || right_type != Type::Bool {
        return Err(positioned(
            format!(
                "type mismatch in '{operator}': expected two booleans, found {} and {}",
                left_type.name(),
                right_type.name()
            ),
            position,
        ));
    }

    Ok(Type::Bool)
}

fn check_binary(
    operator: BinaryOperator,
    left: Type,
    right: Type,
    position: Option<SourcePosition>,
) -> Result<Type, Error> {
    match operator {
        BinaryOperator::Add => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::String, Type::String) => Ok(Type::String),
            _ => Err(positioned(
                format!(
                    "type mismatch in '+': expected two integers or two strings, found {} and {}",
                    left.name(),
                    right.name()
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
            if left == right {
                Ok(Type::Bool)
            } else {
                Err(positioned(
                    format!(
                        "type mismatch in '{symbol}': expected two values of the same type, found {} and {}",
                        left.name(),
                        right.name()
                    ),
                    position,
                ))
            }
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

            if left != Type::Int || right != Type::Int {
                return Err(positioned(
                    format!(
                        "type mismatch in '{symbol}': expected two integers, found {} and {}",
                        left.name(),
                        right.name()
                    ),
                    position,
                ));
            }

            match operator {
                BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Remainder => Ok(Type::Int),
                _ => Ok(Type::Bool),
            }
        }
    }
}

fn positioned(message: impl Into<String>, position: Option<SourcePosition>) -> Error {
    match position {
        Some(position) => Error::at(message, position),
        None => Error::new(message),
    }
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::error::Error;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check_source(input: &str) -> Result<(), Error> {
        let tokens = Lexer::new(input).tokenize()?;
        let program = Parser::new(&tokens).parse()?;
        check(&program)
    }

    fn check_error(input: &str) -> String {
        check_source(input).unwrap_err().to_string()
    }

    fn check_position(input: &str) -> (usize, usize) {
        let error = check_source(input).unwrap_err();
        error
            .position()
            .map(|position| (position.line, position.column))
            .unwrap_or((0, 0))
    }

    #[test]
    fn accepts_well_typed_programs() {
        for program in [
            "1 + 2 * 3",
            "let x = 5; x < 10",
            "true && false || !true",
            "\"a\" + \"b\"",
            "let s = \"hi\"; s == \"hi\"",
            "let x = 5; if x > 3 { x * 2 } else { x }",
            "if true { \"a\" } else { \"b\" }",
            "let flag = true; if flag { 1 } else { 2 }",
            "if (if true { false } else { true }) { 1 } else { 2 }",
            "let x = 1; if true { let x = true; if x { 1 } else { 2 } } else { 3 }",
            "-(1 + 2) * -3",
        ] {
            assert!(check_source(program).is_ok(), "program: {program}");
        }
    }

    #[test]
    fn rejects_mismatched_binary_operands() {
        assert_eq!(
            check_error("1 + true"),
            "type mismatch in '+': expected two integers or two strings, found integer and boolean"
        );
        assert_eq!(
            check_error("\"a\" + 1"),
            "type mismatch in '+': expected two integers or two strings, found string and integer"
        );
        assert_eq!(
            check_error("true && 1"),
            "type mismatch in '&&': expected two booleans, found boolean and integer"
        );
        assert_eq!(
            check_error("1 || true"),
            "type mismatch in '||': expected two booleans, found integer and boolean"
        );
        assert_eq!(
            check_error("1 < true"),
            "type mismatch in '<': expected two integers, found integer and boolean"
        );
        assert_eq!(
            check_error("\"a\" - 1"),
            "type mismatch in '-': expected two integers, found string and integer"
        );
        assert_eq!(
            check_error("1 / \"a\""),
            "type mismatch in '/': expected two integers, found integer and string"
        );
        assert_eq!(
            check_error("true % 2"),
            "type mismatch in '%': expected two integers, found boolean and integer"
        );
    }

    #[test]
    fn rejects_mismatched_equality_operands() {
        assert_eq!(
            check_error("1 == \"a\""),
            "type mismatch in '==': expected two values of the same type, found integer and string"
        );
        assert_eq!(
            check_error("true != 1"),
            "type mismatch in '!=': expected two values of the same type, found boolean and integer"
        );
    }

    #[test]
    fn rejects_mismatched_unary_operands() {
        assert_eq!(
            check_error("-true"),
            "type mismatch in '-': expected an integer, found boolean"
        );
        assert_eq!(
            check_error("!5"),
            "type mismatch in '!': expected a boolean, found integer"
        );
        assert_eq!(
            check_error("!\"a\""),
            "type mismatch in '!': expected a boolean, found string"
        );
    }

    #[test]
    fn rejects_non_boolean_if_conditions() {
        assert_eq!(
            check_error("if 1 { 2 } else { 3 }"),
            "if condition must be a boolean, found integer"
        );
        assert_eq!(
            check_error("if \"a\" { 2 } else { 3 }"),
            "if condition must be a boolean, found string"
        );
    }

    #[test]
    fn rejects_mismatched_if_branches() {
        assert_eq!(
            check_error("if true { 1 } else { \"a\" }"),
            "if branches must have the same type, found integer and string"
        );
        assert_eq!(
            check_error("if true { true } else { 1 }"),
            "if branches must have the same type, found boolean and integer"
        );
    }

    #[test]
    fn rejects_undefined_variables_with_a_position() {
        assert_eq!(check_error("missing + 1"), "undefined variable: 'missing'");
        assert_eq!(check_position("let x = missing; x"), (1, 9));
        assert_eq!(
            check_error("let first = second; let second = 2; first"),
            "undefined variable: 'second'"
        );
    }

    #[test]
    fn comparisons_yield_booleans_and_feed_logical_operators() {
        assert!(check_source("1 < 2").is_ok());
        assert!(check_source("(1 < 2) && (2 < 3)").is_ok());
        assert_eq!(
            check_error("(1 < 2) * 5"),
            "type mismatch in '*': expected two integers, found boolean and integer"
        );
    }

    #[test]
    fn string_concatenation_is_typed_as_string() {
        assert!(check_source("let s = \"a\" + \"b\"; s == \"ab\"").is_ok());
        assert_eq!(
            check_error("(\"a\" + \"b\") * 2"),
            "type mismatch in '*': expected two integers, found string and integer"
        );
    }

    #[test]
    fn type_errors_are_positioned() {
        // The '+' operator of "1 + true" sits at line 1, column 3.
        assert_eq!(check_position("1 + true"), (1, 3));
        // The '!' of "!5" sits at line 1, column 1.
        assert_eq!(check_position("!5"), (1, 1));
    }

    #[test]
    fn shadowed_names_resolve_to_the_innermost_scope() {
        // The inner 'x' is a boolean while the outer 'x' is an integer, so the
        // then branch (boolean) and else branch (integer) cannot agree.
        assert_eq!(
            check_error("let x = 1; if true { let x = true; x } else { x }"),
            "if branches must have the same type, found boolean and integer"
        );
        // Using the shadowed boolean in a boolean position is well typed.
        let program = "let x = 1; if true { let x = true; if x { 1 } else { 2 } } else { 3 }";
        assert!(check_source(program).is_ok());
    }
}
