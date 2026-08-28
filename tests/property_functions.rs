//! A self-contained, dependency-free property-based test that checks the
//! library's evaluation of *function* programs against an independent reference
//! model.
//!
//! Across a deterministic set of seeded cases, a generator builds a well-typed
//! program of `fn` declarations (each with typed parameters and a result type,
//! with an acyclic call graph so evaluation always terminates) plus a top-level
//! body that calls those functions, renders it back to source text, and requires
//! the lexer/parser/type-checker/evaluator pipeline and a straightforward
//! recursive reference evaluator to agree (both succeed with the same value, or
//! both fail). Hand-picked recursive and mutually-recursive programs add
//! guaranteed coverage of self- and mutual recursion.

use std::collections::HashMap;

use rusty_buggy_language::{evaluate, Value};

/// Max absolute value used for generated integer literals, keeping rendering
/// unambiguous (the special `-9223372036854775808` case is never triggered).
const LITERAL_BOUND: i64 = 30;

/// Small strings, including every escape the lexer supports.
const STRING_POOL: &[&str] = &["", "hi", "a b", "new\nline", "tab\there", "say \"x\""];

/// Number of property cases to run per seed.
const CASES_PER_SEED: usize = 300;

/// Upper limit on generated functions per program.
const MAX_FUNCTIONS: usize = 2;
/// Upper limit on parameters per function.
const MAX_PARAMS: usize = 2;

/// Deterministic xorshift64 PRNG so failures are reproducible from the seed.
struct Prng(u64);

impl Prng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: i64) -> i64 {
        (self.next_u64() % (bound as u64)) as i64
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
}

/// The three value types, kept in sync with the language's type checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenType {
    Int,
    Bool,
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Lit(i64),
    BoolLit(bool),
    StrLit(String),
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone)]
struct Function {
    name: String,
    params: Vec<(String, GenType)>,
    result: GenType,
    body: Expr,
}

#[derive(Debug)]
struct Program {
    functions: Vec<Function>,
    declarations: Vec<(String, Expr)>,
    expression: Expr,
}

#[derive(Debug, Clone, PartialEq)]
enum RefValue {
    Int(i64),
    Bool(bool),
    Str(String),
}

/// Precedence used both by the reference model and for minimal-paren rendering.
/// Higher binds tighter. Calls bind like primaries.
fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::Lit(_) | Expr::BoolLit(_) | Expr::StrLit(_) | Expr::Var(_) | Expr::If { .. } => 6,
        Expr::Call { .. } => 6,
        Expr::Neg(_) | Expr::Not(_) => 5,
        Expr::Binary { op, .. } => match op {
            BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder => 4,
            BinaryOp::Add | BinaryOp::Subtract => 3,
            _ => 2,
        },
        Expr::And(_, _) => 1,
        Expr::Or(_, _) => 0,
    }
}

fn wrap(expr: &Expr, threshold: u8) -> String {
    let rendered = render(expr);
    if precedence(expr) < threshold {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn render_string(value: &str) -> String {
    let mut rendered = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => rendered.push_str("\\\\"),
            '\"' => rendered.push_str("\\\""),
            '\n' => rendered.push_str("\\n"),
            '\t' => rendered.push_str("\\t"),
            _ => rendered.push(character),
        }
    }
    rendered.push('"');
    rendered
}

fn render(expr: &Expr) -> String {
    match expr {
        Expr::Lit(value) => value.to_string(),
        Expr::BoolLit(value) => value.to_string(),
        Expr::StrLit(value) => render_string(value),
        Expr::Var(name) => name.clone(),
        Expr::Neg(operand) => format!("-{}", wrap(operand, 5)),
        Expr::Not(operand) => format!("!{}", wrap(operand, 5)),
        Expr::Binary { op, left, right } => {
            let operator = match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                BinaryOp::Remainder => "%",
                BinaryOp::LessThan => "<",
                BinaryOp::LessThanOrEqual => "<=",
                BinaryOp::GreaterThan => ">",
                BinaryOp::GreaterThanOrEqual => ">=",
                BinaryOp::Equal => "==",
                BinaryOp::NotEqual => "!=",
            };
            let this_precedence = precedence(expr);
            let comparison = matches!(
                *op,
                BinaryOp::LessThan
                    | BinaryOp::LessThanOrEqual
                    | BinaryOp::GreaterThan
                    | BinaryOp::GreaterThanOrEqual
                    | BinaryOp::Equal
                    | BinaryOp::NotEqual
            );
            let left_threshold = if comparison {
                this_precedence + 1
            } else {
                this_precedence
            };
            format!(
                "{} {operator} {}",
                wrap(left, left_threshold),
                wrap(right, this_precedence + 1)
            )
        }
        Expr::And(left, right) => format!("{} && {}", wrap(left, 1), wrap(right, 2)),
        Expr::Or(left, right) => format!("{} || {}", wrap(left, 0), wrap(right, 1)),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} {{ {} }} else {{ {} }}",
            render(condition),
            render(then_branch),
            render(else_branch)
        ),
        Expr::Call { name, args } => {
            let args: Vec<String> = args.iter().map(|arg| render(arg)).collect();
            format!("{name}({})", args.join(", "))
        }
    }
}

fn render_program(program: &Program) -> String {
    let mut source = String::new();
    for function in &program.functions {
        let params: Vec<&str> = function.params.iter().map(|(name, _)| name.as_str()).collect();
        source.push_str(&format!(
            "fn {}({}) = {{ {} }}; ",
            function.name,
            params.join(", "),
            render(&function.body)
        ));
    }
    for (name, expr) in &program.declarations {
        source.push_str(&format!("let {name} = {}; ", render(expr)));
    }
    source.push_str(&render(&program.expression));
    source
}

/// The reference evaluator for the whole program: builds a function environment
/// then evaluates the top-level body.
fn reference_program(program: &Program) -> Option<RefValue> {
    let mut functions = HashMap::new();
    for function in &program.functions {
        functions.insert(function.name.clone(), function.clone());
    }

    let mut values = HashMap::new();
    for (name, expr) in &program.declarations {
        values.insert(name.clone(), reference_eval(expr, &values, &functions)?);
    }
    reference_eval(&program.expression, &values, &functions)
}

/// Reference evaluation with plain typed values, mirroring the language's
/// semantics. Returns `None` on failure; callers only compare the Ok/Err
/// decision and, for successes, the value.
fn reference_eval(
    expr: &Expr,
    values: &HashMap<String, RefValue>,
    functions: &HashMap<String, Function>,
) -> Option<RefValue> {
    match expr {
        Expr::Lit(value) => Some(RefValue::Int(*value)),
        Expr::BoolLit(value) => Some(RefValue::Bool(*value)),
        Expr::StrLit(value) => Some(RefValue::Str(value.clone())),
        Expr::Var(name) => values.get(name).cloned(),
        Expr::Neg(operand) => match reference_eval(operand, values, functions)? {
            RefValue::Int(value) => value.checked_neg().map(RefValue::Int),
            _ => None,
        },
        Expr::Not(operand) => match reference_eval(operand, values, functions)? {
            RefValue::Bool(value) => Some(RefValue::Bool(!value)),
            _ => None,
        },
        Expr::And(left, right) => match reference_eval(left, values, functions)? {
            RefValue::Bool(false) => Some(RefValue::Bool(false)),
            RefValue::Bool(true) => match reference_eval(right, values, functions)? {
                RefValue::Bool(value) => Some(RefValue::Bool(value)),
                _ => None,
            },
            _ => None,
        },
        Expr::Or(left, right) => match reference_eval(left, values, functions)? {
            RefValue::Bool(true) => Some(RefValue::Bool(true)),
            RefValue::Bool(false) => match reference_eval(right, values, functions)? {
                RefValue::Bool(value) => Some(RefValue::Bool(value)),
                _ => None,
            },
            _ => None,
        },
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => match reference_eval(condition, values, functions)? {
            RefValue::Bool(true) => reference_eval(then_branch, values, functions),
            RefValue::Bool(false) => reference_eval(else_branch, values, functions),
            _ => None,
        },
        Expr::Call { name, args } => {
            let function = functions.get(name)?;
            if args.len() != function.params.len() {
                return None;
            }
            let mut arg_values = Vec::new();
            for arg in args {
                let value = reference_eval(arg, values, functions)?;
                arg_values.push(value);
            }
            let mut local = HashMap::new();
            for ((param, _), value) in function.params.iter().zip(arg_values.iter()) {
                local.insert(param.clone(), value.clone());
            }
            reference_eval(&function.body, &local, functions)
        }
        Expr::Binary { op, left, right } => {
            let left = reference_eval(left, values, functions)?;
            let right = reference_eval(right, values, functions)?;
            match op {
                BinaryOp::Add => match (left, right) {
                    (RefValue::Int(a), RefValue::Int(b)) => a.checked_add(b).map(RefValue::Int),
                    (RefValue::Str(a), RefValue::Str(b)) => Some(RefValue::Str(a + &b)),
                    _ => None,
                },
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    let equal = match (left, right) {
                        (RefValue::Int(a), RefValue::Int(b)) => a == b,
                        (RefValue::Bool(a), RefValue::Bool(b)) => a == b,
                        (RefValue::Str(a), RefValue::Str(b)) => a == b,
                        _ => return None,
                    };
                    Some(RefValue::Bool(if *op == BinaryOp::Equal {
                        equal
                    } else {
                        !equal
                    }))
                }
                BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder
                | BinaryOp::LessThan
                | BinaryOp::LessThanOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterThanOrEqual => {
                    let (a, b) = match (left, right) {
                        (RefValue::Int(a), RefValue::Int(b)) => (a, b),
                        _ => return None,
                    };
                    match op {
                        BinaryOp::Subtract => a.checked_sub(b).map(RefValue::Int),
                        BinaryOp::Multiply => a.checked_mul(b).map(RefValue::Int),
                        BinaryOp::Divide => {
                            if b == 0 {
                                None
                            } else {
                                a.checked_div(b).map(RefValue::Int)
                            }
                        }
                        BinaryOp::Remainder => {
                            if b == 0 {
                                None
                            } else {
                                a.checked_rem(b).map(RefValue::Int)
                            }
                        }
                        BinaryOp::LessThan => Some(RefValue::Bool(a < b)),
                        BinaryOp::LessThanOrEqual => Some(RefValue::Bool(a <= b)),
                        BinaryOp::GreaterThan => Some(RefValue::Bool(a > b)),
                        BinaryOp::GreaterThanOrEqual => Some(RefValue::Bool(a >= b)),
                        _ => unreachable!("handled above"),
                    }
                }
            }
        }
    }
}

fn to_value(value: &RefValue) -> Value {
    match value {
        RefValue::Int(value) => Value::Int(*value),
        RefValue::Bool(value) => Value::Bool(*value),
        RefValue::Str(value) => Value::String(value.clone()),
    }
}

fn random_type(prng: &mut Prng) -> GenType {
    match prng.below(3) {
        0 => GenType::Int,
        1 => GenType::Bool,
        _ => GenType::Str,
    }
}

/// Generates a random expression of type `target` at or below `depth`.
///
/// `bindings` maps the in-scope variable names (parameters or top-level
/// declarations) to their types; `signatures` holds the callable functions that
/// may be called from this expression (an acyclic subset for function bodies, or
/// all functions from the top level).
fn random_expr(
    prng: &mut Prng,
    depth: u32,
    target: GenType,
    bindings: &[(String, GenType)],
    signatures: &[(String, Vec<GenType>, GenType)],
) -> Expr {
    if depth == 0 || prng.chance(30) {
        let variables: Vec<&(String, GenType)> = bindings
            .iter()
            .filter(|(_, variable_type)| *variable_type == target)
            .collect();
        if !variables.is_empty() && prng.chance(40) {
            let index = prng.below(variables.len() as i64) as usize;
            return Expr::Var(variables[index].0.clone());
        }
        return match target {
            GenType::Int => Expr::Lit(prng.below(LITERAL_BOUND * 2 + 1) - LITERAL_BOUND),
            GenType::Bool => Expr::BoolLit(prng.below(2) == 0),
            GenType::Str => {
                let index = prng.below(STRING_POOL.len() as i64) as usize;
                Expr::StrLit(STRING_POOL[index].to_owned())
            }
        };
    }

    // Occasionally call an available function whose result type matches the
    // target, exercising argument construction and cross-function calls.
    if !signatures.is_empty() && prng.chance(25) {
        let matching: Vec<&(String, Vec<GenType>, GenType)> = signatures
            .iter()
            .filter(|(_, _, result)| *result == target)
            .collect();
        if !matching.is_empty() {
            let (name, params, _) = matching[prng.below(matching.len() as i64) as usize];
            let mut args = Vec::new();
            for param in params {
                args.push(random_expr(prng, depth - 1, *param, bindings, signatures));
            }
            return Expr::Call {
                name: name.clone(),
                args,
            };
        }
    }

    match target {
        GenType::Int => match prng.below(4) {
            0 => Expr::Neg(Box::new(random_expr(
                prng,
                depth - 1,
                GenType::Int,
                bindings,
                signatures,
            ))),
            1 => Expr::Binary {
                op: int_arithmetic_operator(prng),
                left: Box::new(random_expr(prng, depth - 1, GenType::Int, bindings, signatures)),
                right: Box::new(random_expr(
                    prng,
                    depth - 1,
                    GenType::Int,
                    bindings,
                    signatures,
                )),
            },
            2 => random_if(prng, depth, GenType::Int, bindings, signatures),
            _ => random_expr(prng, depth - 1, GenType::Int, bindings, signatures),
        },
        GenType::Bool => match prng.below(6) {
            0 => Expr::Not(Box::new(random_expr(
                prng,
                depth - 1,
                GenType::Bool,
                bindings,
                signatures,
            ))),
            1 => Expr::And(
                Box::new(random_expr(prng, depth - 1, GenType::Bool, bindings, signatures)),
                Box::new(random_expr(prng, depth - 1, GenType::Bool, bindings, signatures)),
            ),
            2 => Expr::Or(
                Box::new(random_expr(prng, depth - 1, GenType::Bool, bindings, signatures)),
                Box::new(random_expr(prng, depth - 1, GenType::Bool, bindings, signatures)),
            ),
            3 => random_int_comparison(prng, depth, bindings, signatures),
            4 => random_equality(prng, depth, bindings, signatures),
            _ => random_if(prng, depth, GenType::Bool, bindings, signatures),
        },
        GenType::Str => match prng.below(3) {
            0 => Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(random_expr(prng, depth - 1, GenType::Str, bindings, signatures)),
                right: Box::new(random_expr(prng, depth - 1, GenType::Str, bindings, signatures)),
            },
            1 => random_if(prng, depth, GenType::Str, bindings, signatures),
            _ => random_expr(prng, depth - 1, GenType::Str, bindings, signatures),
        },
    }
}

fn int_arithmetic_operator(prng: &mut Prng) -> BinaryOp {
    match prng.below(5) {
        0 => BinaryOp::Add,
        1 => BinaryOp::Subtract,
        2 => BinaryOp::Multiply,
        3 => BinaryOp::Divide,
        _ => BinaryOp::Remainder,
    }
}

fn random_int_comparison(
    prng: &mut Prng,
    depth: u32,
    bindings: &[(String, GenType)],
    signatures: &[(String, Vec<GenType>, GenType)],
) -> Expr {
    let op = match prng.below(4) {
        0 => BinaryOp::LessThan,
        1 => BinaryOp::LessThanOrEqual,
        2 => BinaryOp::GreaterThan,
        _ => BinaryOp::GreaterThanOrEqual,
    };
    Expr::Binary {
        op,
        left: Box::new(random_expr(prng, depth - 1, GenType::Int, bindings, signatures)),
        right: Box::new(random_expr(prng, depth - 1, GenType::Int, bindings, signatures)),
    }
}

fn random_equality(
    prng: &mut Prng,
    depth: u32,
    bindings: &[(String, GenType)],
    signatures: &[(String, Vec<GenType>, GenType)],
) -> Expr {
    let operand_type = random_type(prng);
    let op = if prng.below(2) == 0 {
        BinaryOp::Equal
    } else {
        BinaryOp::NotEqual
    };
    Expr::Binary {
        op,
        left: Box::new(random_expr(
            prng,
            depth - 1,
            operand_type,
            bindings,
            signatures,
        )),
        right: Box::new(random_expr(
            prng,
            depth - 1,
            operand_type,
            bindings,
            signatures,
        )),
    }
}

fn random_if(
    prng: &mut Prng,
    depth: u32,
    branch_type: GenType,
    bindings: &[(String, GenType)],
    signatures: &[(String, Vec<GenType>, GenType)],
) -> Expr {
    Expr::If {
        condition: Box::new(random_expr(
            prng,
            depth - 1,
            GenType::Bool,
            bindings,
            signatures,
        )),
        then_branch: Box::new(random_expr(prng, depth - 1, branch_type, bindings, signatures)),
        else_branch: Box::new(random_expr(prng, depth - 1, branch_type, bindings, signatures)),
    }
}

/// Generates a well-typed program of functions plus a top-level body.
///
/// The functions form an acyclic call graph: function `i` may call only
/// functions `0..i`, so the reference evaluation always terminates and the
/// generated program never hits the call-depth guard.
fn random_function_program(prng: &mut Prng) -> Program {
    let function_count = prng.below(MAX_FUNCTIONS as i64 + 1) as usize;
    let mut functions = Vec::new();
    // (name, param types, result type) available to later bodies and the top level.
    let mut global_signatures: Vec<(String, Vec<GenType>, GenType)> = Vec::new();

    for i in 0..function_count {
        let param_count = prng.below(MAX_PARAMS as i64 + 1) as usize;
        let mut params = Vec::new();
        let mut param_types = Vec::new();
        for p in 0..param_count {
            let ty = random_type(prng);
            params.push((format!("p{i}_{p}"), ty));
            param_types.push(ty);
        }
        let result = random_type(prng);

        // A function body may call only earlier functions (acyclic).
        let bindings: Vec<(String, GenType)> = params.clone();
        let body = random_expr(prng, 3, result, &bindings, &global_signatures);

        let name = format!("f{i}");
        functions.push(Function {
            name: name.clone(),
            params,
            result,
            body,
        });
        global_signatures.push((name, param_types, result));
    }

    // Top-level body: a final expression with access to every function and no
    // local declarations, keeping the top level small.
    let target = random_type(prng);
    let expression = random_expr(prng, 4, target, &[], &global_signatures);

    Program {
        functions,
        declarations: Vec::new(),
        expression,
    }
}

/// Asserts that `evaluate(source)` and the reference agree.
fn assert_agrees(source: &str, expected: Option<RefValue>) {
    match (evaluate(source), expected) {
        (Ok(actual), Some(expect)) => assert_eq!(actual, to_value(&expect), "source: {source}"),
        (Err(_), None) => {}
        (Ok(actual), None) => {
            panic!("pipeline succeeded with {actual} but the reference failed: {source}")
        }
        (Err(error), Some(_)) => {
            panic!("reference succeeded but the pipeline failed ({error}): {source}")
        }
    }
}

/// Checks that the generated source is well typed by the pipeline by comparing
/// the pipeline's result with the reference's. Used for the seeded random cases.
fn check_case(source: &str, expected: Option<RefValue>) {
    assert_agrees(source, expected);
}

#[test]
fn generated_function_programs_match_reference_model() {
    let mut prng = Prng::new(0xF17C);
    for _ in 0..CASES_PER_SEED {
        let program = random_function_program(&mut prng);
        let source = render_program(&program);
        let expected = reference_program(&program);
        check_case(&source, expected);
    }
}

/// Fixed recursion programs the generator deliberately avoids producing
/// (recursion can be non-terminating, which the acyclic generator never emits).
fn recursion_cases() -> Vec<(&'static str, Option<RefValue>)> {
    vec![
        (
            "fn fact(n) = { if n <= 1 { 1 } else { n * fact(n - 1) } }; fact(6)",
            Some(RefValue::Int(720)),
        ),
        (
            "fn fib(n) = { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }; fib(8)",
            Some(RefValue::Int(21)),
        ),
        (
            "fn even(n) = { if n == 0 { true } else { odd(n - 1) } }; fn odd(n) = { if n == 0 { false } else { even(n - 1) } }; if even(11) { 1 } else { 0 }",
            Some(RefValue::Int(0)),
        ),
        (
            // Recursion that guards on a boolean flag, returning a string.
            "fn label(n) = { if n <= 1 { \"base\" } else { label(n - 1) + \"!\" } }; label(3)",
            Some(RefValue::Str("base!!".to_owned())),
        ),
    ]
}

#[test]
fn recursion_matches_reference_model() {
    for (source, expected) in recursion_cases() {
        assert_agrees(source, expected);
    }
}
