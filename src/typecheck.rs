//! A static type checker with monomorphic, fixed-point function-type inference.
//!
//! The checker walks the parsed program and rejects ill-typed programs with a
//! positioned error before evaluation. For a program without `fn` declarations
//! this is a single bottom-up pass over the top-level declarations and final
//! expression. For a program with functions, the checker first infers each
//! function's parameter and result types by fixed-point iteration over the
//! small three-type lattice (`integer`, `boolean`, `string`), refining the
//! signatures monotonically from each function's body and from every call site
//! (including recursive and top-level calls) until they stabilize.
//!
//! Inference is bottom-up but carries *provenance* for unknown types: an
//! `Unknown` flow keeps a reference to the exact parameter slot or callee-result
//! slot it came from, so a concrete demand in an enclosing operator can pin that
//! slot and the value "flows" upward. A type that no body or call site ever pins
//! stays `Unknown` after convergence and is rejected as an inference failure.

use std::collections::HashMap;

use crate::ast::{
    BinaryOperator, Block, Declaration, Expression, Function, FunctionDeclaration, Program, Type,
};
use crate::error::{Error, SourcePosition};

/// The lattice used for inference. `Unknown` is the least element, the three
/// concrete types sit above it, and `Mixed` is the top element denoting two
/// incompatible demands made on one slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lattice {
    Unknown,
    Concrete(Type),
    Mixed,
}

/// Least upper bound under the inference order.
fn join(left: Lattice, right: Lattice) -> Lattice {
    use Lattice::*;
    match (left, right) {
        (Unknown, other) | (other, Unknown) => other,
        (Mixed, _) | (_, Mixed) => Mixed,
        (Concrete(a), Concrete(b)) => {
            if a == b {
                Concrete(a)
            } else {
                Mixed
            }
        }
    }
}

/// The result of inferring an expression's type. `Concrete` and `Mixed` are
/// settled; an `Unknown` element keeps the provenance of where it came from so a
/// later concrete demand can pin the exact slot.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Flow {
    Concrete(Type),
    Mixed,
    Unknown(Where),
}

/// Where an unknown type lives. `Param(fi, pi)` is the `pi`-th parameter of
/// function `fi`; `Result(fi)` is the result slot of function `fi`. `Aggregate`
/// combines several unknowns (for example `a + b` with both unknown) and cannot
/// be routed to a single slot, so it stays genuinely ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Where {
    Param(usize, usize),
    Result(usize),
    Aggregate,
}

impl Flow {
    fn lattice(self) -> Lattice {
        match self {
            Flow::Concrete(ty) => Lattice::Concrete(ty),
            Flow::Mixed => Lattice::Mixed,
            Flow::Unknown(_) => Lattice::Unknown,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Flow::Concrete(ty) => ty.name(),
            Flow::Mixed => "mixed types",
            Flow::Unknown(_) => "an unresolved type",
        }
    }

    fn is_concrete(self) -> bool {
        matches!(self, Flow::Concrete(_))
    }
}

/// A signature being refined during inference.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Signature {
    name: String,
    parameter_types: Vec<Lattice>,
    result_type: Lattice,
}

/// The resolved function bindings used by the evaluator: a lookup from each
/// function's name to its declaration plus resolved parameter and result types.
pub(crate) type ResolvedFunctions = HashMap<String, Function>;

/// Public entry point: type-checks a program, returning the first positioned
/// type error.
pub(crate) fn check(program: &Program) -> Result<(), Error> {
    resolve(program).map(|_| ())
}

/// Type-checks the program and returns the resolved function bindings (the
/// [`ResolvedFunctions`] the evaluator uses to run calls). Doing both in one
/// call keeps inference single-pass and guarantees the evaluator and the
/// checker see the same resolved signatures.
pub(crate) fn resolve(program: &Program) -> Result<ResolvedFunctions, Error> {
    structural_checks(program)?;

    let mut signatures = initial_signatures(&program.functions);

    if program.functions.is_empty() {
        check_top_level(program, &mut signatures)?;
        debug_assert!(signatures.is_empty());
        return Ok(HashMap::new());
    }

    run_inference(program, &mut signatures)?;
    validate_resolved(program, &signatures)?;

    let functions: HashMap<String, Function> = program
        .functions
        .iter()
        .zip(signatures.iter())
        .map(|(declaration, signature)| {
            let function = Function {
                name: declaration.name.clone(),
                parameters: declaration.parameters.clone(),
                parameter_types: signature
                    .parameter_types
                    .iter()
                    .map(|lattice| expect_concrete(*lattice, "parameter"))
                    .collect(),
                result_type: expect_concrete(signature.result_type, "result"),
                body: declaration.body.clone(),
            };
            (function.name.clone(), function)
        })
        .collect();

    Ok(functions)
}

fn expect_concrete(lattice: Lattice, kind: &str) -> Type {
    match lattice {
        Lattice::Concrete(ty) => ty,
        _ => panic!("validated signature slot ({kind}) was {lattice:?}"),
    }
}

fn initial_signatures(functions: &[FunctionDeclaration]) -> Vec<Signature> {
    functions
        .iter()
        .map(|function| Signature {
            name: function.name.clone(),
            parameter_types: vec![Lattice::Unknown; function.parameters.len()],
            result_type: Lattice::Unknown,
        })
        .collect()
}

/// Validates the structural properties that inference cannot repair: every call
/// resolves to a declared function with the right number of arguments. Runs
/// before inference so call-shape errors are reported deterministically.
fn structural_checks(program: &Program) -> Result<(), Error> {
    let names: Vec<&str> = program.functions.iter().map(|f| f.name.as_str()).collect();
    for function in &program.functions {
        check_expression_calls(&function.body.expression, &function.body.declarations, &names)?;
    }
    check_expression_calls(&program.expression, &program.declarations, &names)
}

fn check_expression_calls(
    expression: &Expression,
    declarations: &[Declaration],
    names: &[&str],
) -> Result<(), Error> {
    check_expression_shape(expression, names)?;
    for declaration in declarations {
        check_expression_shape(&declaration.initializer, names)?;
    }
    Ok(())
}

fn check_expression_shape(expression: &Expression, names: &[&str]) -> Result<(), Error> {
    match expression {
        Expression::Call {
            callee,
            arguments,
            position,
        } => {
            if !names.contains(&callee.as_str()) {
                return Err(positioned(format!("undefined function: '{callee}'"), *position));
            }
            for argument in arguments {
                check_expression_shape(argument, names)?;
            }
        }
        Expression::UnaryNegation { operand, .. } | Expression::UnaryNot { operand, .. } => {
            check_expression_shape(operand, names)?;
        }
        Expression::Binary { left, right, .. }
        | Expression::LogicalAnd { left, right, .. }
        | Expression::LogicalOr { left, right, .. } => {
            check_expression_shape(left, names)?;
            check_expression_shape(right, names)?;
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            check_expression_shape(condition, names)?;
            check_block_shape(then_branch, names)?;
            check_block_shape(else_branch, names)?;
        }
        _ => {}
    }
    Ok(())
}

fn check_block_shape(block: &Block, names: &[&str]) -> Result<(), Error> {
    check_expression_calls(&block.expression, &block.declarations, names)
}

/// Runs the fixed-point inference. Each pass re-derives every signature from the
/// function bodies (including recursive calls) and the top-level body, joining
/// demands upward. The lattice is finite and every update is a join, so the
/// process converges; the pass cap is a generous bound no real program reaches.
fn run_inference(program: &Program, signatures: &mut Vec<Signature>) -> Result<(), Error> {
    let max_passes = signatures.len() * 8 + 32;

    for _ in 0..max_passes {
        let snapshot = signatures.clone();

        for (fi, function) in program.functions.iter().enumerate() {
            let mut params = HashMap::new();
            for (pi, name) in function.parameters.iter().enumerate() {
                params.insert(name.clone(), ParamRef { function: fi, index: pi });
            }
            let mut locals = HashMap::new();
            let body_flow = check_block_flow(&function.body, &params, &mut locals, signatures)?;
            signatures[fi].result_type = join(signatures[fi].result_type, body_flow.lattice());
        }

        check_top_level(program, signatures)?;

        if signatures == &snapshot {
            return Ok(());
        }
    }

    Err(Error::new("could not infer the types of the declared functions"))
}

/// Checks the top-level declarations and final expression. For programs without
/// functions this is the only pass; for programs with functions it runs on every
/// pass so top-level calls pin callee signatures.
fn check_top_level(program: &Program, signatures: &mut [Signature]) -> Result<(), Error> {
    let params = Params::default();
    let mut locals = HashMap::new();
    check_declarations_and_expr(
        &program.declarations,
        &program.expression,
        &params,
        &mut locals,
        signatures,
    )?;
    Ok(())
}

/// Validates that every resolved signature slot is a concrete type, rejecting
/// parameters and results that no body or call site ever pinned.
fn validate_resolved(program: &Program, signatures: &[Signature]) -> Result<(), Error> {
    for (fi, function) in program.functions.iter().enumerate() {
        for (pi, parameter) in function.parameters.iter().enumerate() {
            match signatures[fi].parameter_types[pi] {
                Lattice::Concrete(_) => {}
                Lattice::Unknown => {
                    return Err(positioned(
                        format!(
                            "cannot infer the type of parameter '{parameter}' in function '{}'",
                            function.name
                        ),
                        function.position,
                    ))
                }
                Lattice::Mixed => {
                    return Err(positioned(
                        format!(
                            "cannot infer a consistent type for parameter '{parameter}' in function '{}'",
                            function.name
                        ),
                        function.position,
                    ))
                }
            }
        }
        match signatures[fi].result_type {
            Lattice::Concrete(_) => {}
            Lattice::Unknown => {
                return Err(positioned(
                    format!(
                        "cannot infer the result type of function '{}'",
                        function.name
                    ),
                    function.position,
                ))
            }
            Lattice::Mixed => {
                return Err(positioned(
                    format!(
                        "cannot infer a consistent type for function '{}'",
                        function.name
                    ),
                    function.position,
                ))
            }
        }
    }
    Ok(())
}

/// Maps a parameter name to the signature slot that owns it.
#[derive(Clone, Copy)]
struct ParamRef {
    function: usize,
    index: usize,
}

type Params = HashMap<String, ParamRef>;
type Locals = HashMap<String, Lattice>;

fn check_declarations_and_expr(
    declarations: &[Declaration],
    expression: &Expression,
    params: &Params,
    locals: &mut Locals,
    signatures: &mut [Signature],
) -> Result<(), Error> {
    for declaration in declarations {
        let flow = check_expr_flow(&declaration.initializer, params, locals, signatures)?;
        locals.insert(declaration.name.clone(), flow.lattice());
    }
    check_expr_flow(expression, params, locals, signatures)?;
    Ok(())
}

fn check_block_flow(
    block: &Block,
    params: &Params,
    locals: &mut Locals,
    signatures: &mut [Signature],
) -> Result<Flow, Error> {
    // A function body or an if/else block: local declarations shadow in a fresh
    // scope that does not leak out.
    let mut inner = locals.clone();
    for declaration in &block.declarations {
        let flow = check_expr_flow(&declaration.initializer, params, &mut inner, signatures)?;
        inner.insert(declaration.name.clone(), flow.lattice());
    }
    check_expr_flow(&block.expression, params, &mut inner, signatures)
}

fn check_expr_flow(
    expression: &Expression,
    params: &Params,
    locals: &mut Locals,
    signatures: &mut [Signature],
) -> Result<Flow, Error> {
    match expression {
        Expression::Literal { .. } => Ok(Flow::Concrete(Type::Int)),
        Expression::StringLiteral { .. } => Ok(Flow::Concrete(Type::String)),
        Expression::BoolLiteral { .. } => Ok(Flow::Concrete(Type::Bool)),
        Expression::Variable { name, position } => {
            if let Some(ParamRef { function, index }) = params.get(name) {
                return Ok(match signatures[*function].parameter_types[*index] {
                    Lattice::Unknown => Flow::Unknown(Where::Param(*function, *index)),
                    Lattice::Concrete(ty) => Flow::Concrete(ty),
                    Lattice::Mixed => Flow::Mixed,
                });
            }
            if let Some(level) = locals.get(name) {
                return Ok(match level {
                    Lattice::Unknown => Flow::Unknown(Where::Aggregate),
                    Lattice::Concrete(ty) => Flow::Concrete(*ty),
                    Lattice::Mixed => Flow::Mixed,
                });
            }
            Err(positioned(format!("undefined variable: '{name}'"), *position))
        }
        Expression::Call {
            callee,
            arguments,
            position,
        } => {
            let fi = signatures
                .iter()
                .position(|signature| signature.name == *callee)
                .ok_or_else(|| positioned(format!("undefined function: '{callee}'"), *position))?;
            let arity = signatures[fi].parameter_types.len();
            if arguments.len() != arity {
                return Err(positioned(
                    format!(
                        "wrong number of arguments for function '{callee}': expected {arity}, found {}",
                        arguments.len()
                    ),
                    *position,
                ));
            }
            for (index, argument) in arguments.iter().enumerate() {
                let argument_flow = check_expr_flow(argument, params, locals, signatures)?;
                let slot = &mut signatures[fi].parameter_types[index];
                match (*slot, argument_flow) {
                    // The callee's parameter type is already settled to a
                    // different type (from its body or another call site):
                    // report the mismatch at the call site.
                    (Lattice::Concrete(slot_type), Flow::Concrete(argument_type))
                        if slot_type != argument_type =>
                    {
                        return Err(positioned(
                            format!(
                                "type mismatch in call to '{callee}': expected argument {} to be {}, found {}",
                                index + 1,
                                slot_type.name(),
                                argument_type.name()
                            ),
                            *position,
                        ));
                    }
                    _ => {
                        *slot = join(*slot, argument_flow.lattice());
                    }
                }
            }
            Ok(match signatures[fi].result_type {
                Lattice::Unknown => Flow::Unknown(Where::Result(fi)),
                Lattice::Concrete(ty) => Flow::Concrete(ty),
                Lattice::Mixed => Flow::Mixed,
            })
        }
        Expression::UnaryNegation { operand, position } => {
            let operand_flow = check_expr_flow(operand, params, locals, signatures)?;
            require_unary(operand_flow, "-", Type::Int, *position, signatures)?;
            Ok(Flow::Concrete(Type::Int))
        }
        Expression::UnaryNot { operand, position } => {
            let operand_flow = check_expr_flow(operand, params, locals, signatures)?;
            require_unary(operand_flow, "!", Type::Bool, *position, signatures)?;
            Ok(Flow::Concrete(Type::Bool))
        }
        Expression::LogicalAnd { left, right, position } => {
            check_boolean_pair("&&", left, right, *position, params, locals, signatures)
        }
        Expression::LogicalOr { left, right, position } => {
            check_boolean_pair("||", left, right, *position, params, locals, signatures)
        }
        Expression::Binary {
            operator,
            left,
            right,
            position,
        } => check_binary(*operator, left, right, *position, params, locals, signatures),
        Expression::If {
            condition,
            then_branch,
            else_branch,
            position,
        } => {
            let condition_flow = check_expr_flow(condition, params, locals, signatures)?;
            require_if_condition(condition_flow, *position, signatures)?;

            let then_flow = check_block_flow(then_branch, params, locals, signatures)?;
            let else_flow = check_block_flow(else_branch, params, locals, signatures)?;
            unify_branches(then_flow, else_flow, *position)
        }
    }
}

/// Pins any unknown slot to `required` (a settled operand or conflict is
/// untouched). Returns nothing; the caller is responsible for conflict reports.
fn pin_type(flow: Flow, required: Type, signatures: &mut [Signature]) {
    match flow {
        Flow::Unknown(Where::Param(fi, pi)) => {
            signatures[fi].parameter_types[pi] =
                join(signatures[fi].parameter_types[pi], Lattice::Concrete(required));
        }
        Flow::Unknown(Where::Result(fi)) => {
            signatures[fi].result_type = join(signatures[fi].result_type, Lattice::Concrete(required));
        }
        _ => {}
    }
}

/// Whether a flow is a settled type other than the allowed `concrete` type.
fn is_bad_operand(flow: Flow, required: Type) -> bool {
    match flow {
        Flow::Concrete(ty) => ty != required,
        Flow::Mixed => true,
        Flow::Unknown(_) => false,
    }
}

fn require_unary(
    flow: Flow,
    operator: &str,
    required: Type,
    position: Option<SourcePosition>,
    signatures: &mut [Signature],
) -> Result<(), Error> {
    let expected = match required {
        Type::Int => "an integer",
        Type::Bool => "a boolean",
        Type::String => "a string",
    };
    if is_bad_operand(flow, required) {
        return Err(positioned(
            format!(
                "type mismatch in '{operator}': expected {expected}, found {}",
                flow.name()
            ),
            position,
        ));
    }
    pin_type(flow, required, signatures);
    Ok(())
}

fn require_if_condition(
    flow: Flow,
    position: Option<SourcePosition>,
    signatures: &mut [Signature],
) -> Result<(), Error> {
    if is_bad_operand(flow, Type::Bool) {
        return Err(positioned(
            format!("if condition must be a boolean, found {}", flow.name()),
            position,
        ));
    }
    pin_type(flow, Type::Bool, signatures);
    Ok(())
}

fn require_two(
    left_flow: Flow,
    right_flow: Flow,
    required: Type,
    family: &str,
    symbol: &str,
    position: Option<SourcePosition>,
    signatures: &mut [Signature],
) -> Result<(), Error> {
    let bad = is_bad_operand(left_flow, required) || is_bad_operand(right_flow, required);
    pin_type(left_flow, required, signatures);
    pin_type(right_flow, required, signatures);
    if bad {
        return Err(positioned(
            format!(
                "type mismatch in '{symbol}': expected two {family}, found {} and {}",
                left_flow.name(),
                right_flow.name()
            ),
            position,
        ));
    }
    Ok(())
}

fn check_boolean_pair(
    operator: &str,
    left: &Expression,
    right: &Expression,
    position: Option<SourcePosition>,
    params: &Params,
    locals: &mut Locals,
    signatures: &mut [Signature],
) -> Result<Flow, Error> {
    let left_flow = check_expr_flow(left, params, locals, signatures)?;
    let right_flow = check_expr_flow(right, params, locals, signatures)?;
    require_two(
        left_flow,
        right_flow,
        Type::Bool,
        "booleans",
        operator,
        position,
        signatures,
    )?;
    Ok(Flow::Concrete(Type::Bool))
}

fn check_binary(
    operator: BinaryOperator,
    left: &Expression,
    right: &Expression,
    position: Option<SourcePosition>,
    params: &Params,
    locals: &mut Locals,
    signatures: &mut [Signature],
) -> Result<Flow, Error> {
    let left_flow = check_expr_flow(left, params, locals, signatures)?;
    let right_flow = check_expr_flow(right, params, locals, signatures)?;

    match operator {
        BinaryOperator::Add => check_add(left_flow, right_flow, position, signatures),
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let symbol = if operator == BinaryOperator::Equal {
                "=="
            } else {
                "!="
            };
            // Equality/inequality accept two values of the same type without
            // imposing a specific one; only a known mismatch is an error.
            if left_flow.is_concrete() && right_flow.is_concrete() && left_flow != right_flow {
                return Err(positioned(
                    format!(
                        "type mismatch in '{symbol}': expected two values of the same type, found {} and {}",
                        left_flow.name(),
                        right_flow.name()
                    ),
                    position,
                ));
            }
            Ok(Flow::Concrete(Type::Bool))
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
            require_two(
                left_flow,
                right_flow,
                Type::Int,
                "integers",
                symbol,
                position,
                signatures,
            )?;
            Ok(match operator {
                BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Remainder => Flow::Concrete(Type::Int),
                _ => Flow::Concrete(Type::Bool),
            })
        }
    }
}

fn check_add(
    left_flow: Flow,
    right_flow: Flow,
    position: Option<SourcePosition>,
    signatures: &mut [Signature],
) -> Result<Flow, Error> {
    // `+` is addition (Int, Int) or concatenation (String, String). Decide the
    // intended type from any concrete operand.
    if left_flow == Flow::Mixed || right_flow == Flow::Mixed {
        return Err(positioned(
            format!(
                "type mismatch in '+': expected two integers or two strings, found {} and {}",
                left_flow.name(),
                right_flow.name()
            ),
            position,
        ));
    }

    let left_type = concrete_type(left_flow);
    let right_type = concrete_type(right_flow);
    let intended = match (left_type, right_type) {
        (Some(a), Some(b)) => {
            if a != b {
                return Err(positioned(
                    format!(
                        "type mismatch in '+': expected two integers or two strings, found {} and {}",
                        left_flow.name(),
                        right_flow.name()
                    ),
                    position,
                ));
            }
            a
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => {
            if left_flow == Flow::Mixed || right_flow == Flow::Mixed {
                return Err(positioned(
                    format!(
                        "type mismatch in '+': expected two integers or two strings, found {} and {}",
                        left_flow.name(),
                        right_flow.name()
                    ),
                    position,
                ));
            }
            // Both unknown: genuinely ambiguous between addition and
            // concatenation, so leave it unresolved.
            return Ok(Flow::Unknown(Where::Aggregate));
        }
    };
    pin_type(left_flow, intended, signatures);
    pin_type(right_flow, intended, signatures);
    Ok(Flow::Concrete(intended))
}

fn concrete_type(flow: Flow) -> Option<Type> {
    match flow {
        Flow::Concrete(ty) => Some(ty),
        _ => None,
    }
}

fn unify_branches(
    then_flow: Flow,
    else_flow: Flow,
    position: Option<SourcePosition>,
) -> Result<Flow, Error> {
    if then_flow.is_concrete() && else_flow.is_concrete() {
        if then_flow != else_flow {
            return Err(positioned(
                format!(
                    "if branches must have the same type, found {} and {}",
                    then_flow.name(),
                    else_flow.name()
                ),
                position,
            ));
        }
        return Ok(then_flow);
    }
    if then_flow == Flow::Mixed || else_flow == Flow::Mixed {
        return Err(positioned(
            format!(
                "if branches must have the same type, found {} and {}",
                then_flow.name(),
                else_flow.name()
            ),
            position,
        ));
    }
    // One or both branches are unknown; unify to the concrete side when exactly
    // one is known, otherwise stay unknown for a caller or inference validation
    // to resolve or reject.
    Ok(match (then_flow, else_flow) {
        (Flow::Unknown(_), Flow::Concrete(ty)) => Flow::Concrete(ty),
        (Flow::Concrete(ty), Flow::Unknown(_)) => Flow::Concrete(ty),
        _ => Flow::Unknown(Where::Aggregate),
    })
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

    fn accepts(program: &str) {
        assert!(check_source(program).is_ok(), "program: {program}");
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
            check_source(program).expect(program);
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
        assert_eq!(check_error("!5"), "type mismatch in '!': expected a boolean, found integer");
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
        accepts("1 < 2");
        accepts("(1 < 2) && (2 < 3)");
        assert_eq!(
            check_error("(1 < 2) * 5"),
            "type mismatch in '*': expected two integers, found boolean and integer"
        );
    }

    #[test]
    fn string_concatenation_is_typed_as_string() {
        accepts("let s = \"a\" + \"b\"; s == \"ab\"");
        assert_eq!(
            check_error("(\"a\" + \"b\") * 2"),
            "type mismatch in '*': expected two integers, found string and integer"
        );
    }

    #[test]
    fn type_errors_are_positioned() {
        assert_eq!(check_position("1 + true"), (1, 3));
        assert_eq!(check_position("!5"), (1, 1));
    }

    #[test]
    fn shadowed_names_resolve_to_the_innermost_scope() {
        assert_eq!(
            check_error("let x = 1; if true { let x = true; x } else { x }"),
            "if branches must have the same type, found boolean and integer"
        );
        accepts("let x = 1; if true { let x = true; if x { 1 } else { 2 } } else { 3 }");
    }

    // --- Function inference ---

    #[test]
    fn infers_parameter_and_result_types_from_a_body() {
        accepts("fn sq(x) = { x * x }; sq(3)");
        accepts("fn abs(x) = { if x < 0 { -x } else { x } }; abs(-5)");
    }

    #[test]
    fn infers_types_from_call_sites_when_the_body_is_pass_through() {
        // The parameter type of `id` is pinned by the top-level call.
        accepts("fn id(x) = { x }; id(5)");
        accepts("fn id(x) = { x }; id(\"hi\")");
    }

    #[test]
    fn reasons_recursively() {
        accepts("fn fact(n) = { if n <= 1 { 1 } else { n * fact(n - 1) } }; fact(5)");
        accepts(
            "fn even(n) = { if n == 0 { true } else { odd(n - 1) } }; fn odd(n) = { if n == 0 { false } else { even(n - 1) } }; even(10)",
        );
    }

    #[test]
    fn functions_can_return_blocks_with_local_declarations() {
        accepts("fn max(a, b) = { let big = if a > b { a } else { b }; big }; max(3, 7)");
    }

    #[test]
    fn rejects_undefined_functions() {
        assert_eq!(check_error("missing(1)"), "undefined function: 'missing'");
        assert_eq!(check_position("let x = 1; goo(x)"), (1, 12));
    }

    #[test]
    fn rejects_wrong_argument_counts() {
        assert_eq!(
            check_error("fn f(a, b) = { a + b }; f(1)"),
            "wrong number of arguments for function 'f': expected 2, found 1"
        );
        assert_eq!(
            check_error("fn f(a) = { a }; f(1, 2)"),
            "wrong number of arguments for function 'f': expected 1, found 2"
        );
    }

    #[test]
    fn rejects_call_site_type_mismatches() {
        assert_eq!(
            check_error("fn double(x) = { x * 2 }; double(true)"),
            "type mismatch in call to 'double': expected argument 1 to be integer, found boolean"
        );
        assert_eq!(
            check_error("fn takes_int(x) = { if x > 0 { x } else { 0 } }; takes_int(true)"),
            "type mismatch in call to 'takes_int': expected argument 1 to be integer, found boolean"
        );
    }

    #[test]
    fn rejects_unresolvable_inference() {
        // A pass-through parameter with nothing to pin it cannot be inferred.
        assert!(check_error("fn id(x) = { x }; 1").contains("cannot infer"));
        // A never-called function whose parameters are only passed through is
        // ambiguous and must be rejected rather than silently defaulted.
        assert!(check_error("fn choose(a, b) = { if true { a } else { b } }; 1").contains("cannot infer"));
    }
}