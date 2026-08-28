//! A self-contained, dependency-free property-based test that checks the
//! library's evaluation against an independent reference model.
//!
//! For a deterministic number of random cases, a generator builds a
//! well-typed expression (or a program of immutable `let` bindings ending in
//! an expression) over integers, booleans, and strings, renders it back to
//! source text with minimal parentheses that respect the language's
//! precedence and associativity, and then requires the
//! lexer/parser/type-checker/evaluator pipeline and a straightforward
//! recursive reference evaluator to agree: either both succeed with the same
//! [`Value`], or both fail. Being dependency-free, the test runs identically
//! on the MSRV.

use std::collections::HashMap;

use rusty_buggy_language::{evaluate, Value};

/// Max absolute value used for generated integer literals. Staying well below
/// `i64::MAX` keeps rendering unambiguous: literals print with an explicit
/// sign and the special `-9223372036854775808` case is never triggered.
const LITERAL_BOUND: i64 = 50;

/// Small strings used for generated string literals, including every escape
/// the lexer supports so the render/parse round trip is exercised.
const STRING_POOL: &[&str] = &[
    "",
    "hello",
    "a b",
    "say \"hi\"",
    "tab\there",
    "new\nline",
    "back\\slash",
];

/// The number of declaration expressions generated ahead of the final one.
const MAX_DECLARATIONS: usize = 3;

/// Number of property cases to run per seed. Kept modest so the suite stays
/// fast; each case is an independent evaluation.
const CASES_PER_SEED: usize = 400;

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

/// The type a generated expression must have, kept in sync with the language's
/// static type checker so generated programs are well typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenType {
    Int,
    Bool,
    Str,
}

#[derive(Debug, Clone, PartialEq)]
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
}

#[derive(Debug, Clone)]
struct Program {
    declarations: Vec<(String, Expr)>,
    expression: Expr,
}

#[derive(Debug, Clone, PartialEq)]
enum RefValue {
    Int(i64),
    Bool(bool),
    Str(String),
}

/// Precedence used both by the reference model and for minimal-paren
/// rendering. Higher binds tighter.
fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::Lit(_) | Expr::BoolLit(_) | Expr::StrLit(_) | Expr::Var(_) | Expr::If { .. } => 6,
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

/// Renders a string literal with the escapes the lexer accepts.
fn render_string(value: &str) -> String {
    let mut rendered = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => rendered.push_str("\\\\"),
            '"' => rendered.push_str("\\\""),
            '\n' => rendered.push_str("\\n"),
            '\t' => rendered.push_str("\\t"),
            _ => rendered.push(character),
        }
    }
    rendered.push('"');
    rendered
}

/// Renders an expression with the minimum parentheses needed to preserve its
/// tree under the language's precedence and associativity rules. Binary
/// operators are left-associative; negation and not are right-associative.
/// Comparisons cannot be chained, so a comparison operand of a comparison is
/// parenthesized on both sides.
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
    }
}

/// Reference evaluation with plain typed values, mirroring the language's
/// semantics: checked `i64` arithmetic, string concatenation, short-circuiting
/// logical operators, and `if`/`else` branch selection. Returns `None` when
/// evaluation fails; callers only compare the Ok/Err decision and, for
/// successes, the value.
fn reference_eval(expr: &Expr, values: &HashMap<String, RefValue>) -> Option<RefValue> {
    match expr {
        Expr::Lit(value) => Some(RefValue::Int(*value)),
        Expr::BoolLit(value) => Some(RefValue::Bool(*value)),
        Expr::StrLit(value) => Some(RefValue::Str(value.clone())),
        Expr::Var(name) => values.get(name).cloned(),
        Expr::Neg(operand) => match reference_eval(operand, values)? {
            RefValue::Int(value) => value.checked_neg().map(RefValue::Int),
            _ => None,
        },
        Expr::Not(operand) => match reference_eval(operand, values)? {
            RefValue::Bool(value) => Some(RefValue::Bool(!value)),
            _ => None,
        },
        Expr::And(left, right) => match reference_eval(left, values)? {
            RefValue::Bool(false) => Some(RefValue::Bool(false)),
            RefValue::Bool(true) => match reference_eval(right, values)? {
                RefValue::Bool(value) => Some(RefValue::Bool(value)),
                _ => None,
            },
            _ => None,
        },
        Expr::Or(left, right) => match reference_eval(left, values)? {
            RefValue::Bool(true) => Some(RefValue::Bool(true)),
            RefValue::Bool(false) => match reference_eval(right, values)? {
                RefValue::Bool(value) => Some(RefValue::Bool(value)),
                _ => None,
            },
            _ => None,
        },
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => match reference_eval(condition, values)? {
            RefValue::Bool(true) => reference_eval(then_branch, values),
            RefValue::Bool(false) => reference_eval(else_branch, values),
            _ => None,
        },
        Expr::Binary { op, left, right } => {
            let left = reference_eval(left, values)?;
            let right = reference_eval(right, values)?;

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

fn reference_program(program: &Program) -> Option<RefValue> {
    let mut values = HashMap::new();
    for (name, expr) in &program.declarations {
        values.insert(name.clone(), reference_eval(expr, &values)?);
    }
    reference_eval(&program.expression, &values)
}

fn to_value(value: &RefValue) -> Value {
    match value {
        RefValue::Int(value) => Value::Int(*value),
        RefValue::Bool(value) => Value::Bool(*value),
        RefValue::Str(value) => Value::String(value.clone()),
    }
}

/// Generates a random expression of the target type and the given depth.
/// `names` maps declared identifiers to their types; an empty slice means no
/// variables may be referenced.
fn random_expr(prng: &mut Prng, depth: u32, target: GenType, names: &[(String, GenType)]) -> Expr {
    if depth == 0 || prng.chance(30) {
        // Leaf: a variable of the target type when one exists, otherwise a
        // literal of the target type.
        let variables: Vec<&(String, GenType)> = names
            .iter()
            .filter(|(_, variable_type)| *variable_type == target)
            .collect();
        if !variables.is_empty() && prng.chance(45) {
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

    match target {
        GenType::Int => match prng.below(5) {
            0 => Expr::Neg(Box::new(random_expr(prng, depth - 1, GenType::Int, names))),
            1 => Expr::Binary {
                op: int_arithmetic_operator(prng),
                left: Box::new(random_expr(prng, depth - 1, GenType::Int, names)),
                right: Box::new(random_expr(prng, depth - 1, GenType::Int, names)),
            },
            2 => random_if(prng, depth, GenType::Int, names),
            _ => random_expr(prng, depth - 1, GenType::Int, names),
        },
        GenType::Bool => match prng.below(7) {
            0 => Expr::Not(Box::new(random_expr(prng, depth - 1, GenType::Bool, names))),
            1 => Expr::And(
                Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
                Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
            ),
            2 => Expr::Or(
                Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
                Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
            ),
            3 => random_int_comparison(prng, depth, names),
            4 => random_equality(prng, depth, names),
            5 => random_if(prng, depth, GenType::Bool, names),
            _ => random_expr(prng, depth - 1, GenType::Bool, names),
        },
        GenType::Str => match prng.below(4) {
            0 => Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(random_expr(prng, depth - 1, GenType::Str, names)),
                right: Box::new(random_expr(prng, depth - 1, GenType::Str, names)),
            },
            1 => random_if(prng, depth, GenType::Str, names),
            _ => random_expr(prng, depth - 1, GenType::Str, names),
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

/// A `<`, `<=`, `>`, or `>=` comparison of two integers, yielding a boolean.
fn random_int_comparison(prng: &mut Prng, depth: u32, names: &[(String, GenType)]) -> Expr {
    let op = match prng.below(4) {
        0 => BinaryOp::LessThan,
        1 => BinaryOp::LessThanOrEqual,
        2 => BinaryOp::GreaterThan,
        _ => BinaryOp::GreaterThanOrEqual,
    };
    Expr::Binary {
        op,
        left: Box::new(random_expr(prng, depth - 1, GenType::Int, names)),
        right: Box::new(random_expr(prng, depth - 1, GenType::Int, names)),
    }
}

/// An `==` or `!=` comparison of two values of the same random type.
fn random_equality(prng: &mut Prng, depth: u32, names: &[(String, GenType)]) -> Expr {
    let operand_type = match prng.below(3) {
        0 => GenType::Int,
        1 => GenType::Bool,
        _ => GenType::Str,
    };
    let op = if prng.below(2) == 0 {
        BinaryOp::Equal
    } else {
        BinaryOp::NotEqual
    };
    Expr::Binary {
        op,
        left: Box::new(random_expr(prng, depth - 1, operand_type, names)),
        right: Box::new(random_expr(prng, depth - 1, operand_type, names)),
    }
}

/// An `if` expression whose condition is boolean and whose branches both have
/// the given type, keeping the program well typed.
fn random_if(
    prng: &mut Prng,
    depth: u32,
    branch_type: GenType,
    names: &[(String, GenType)],
) -> Expr {
    Expr::If {
        condition: Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
        then_branch: Box::new(random_expr(prng, depth - 1, branch_type, names)),
        else_branch: Box::new(random_expr(prng, depth - 1, branch_type, names)),
    }
}

fn declaration_name(index: usize) -> String {
    format!("value{index}")
}

fn random_type(prng: &mut Prng) -> GenType {
    match prng.below(3) {
        0 => GenType::Int,
        1 => GenType::Bool,
        _ => GenType::Str,
    }
}

fn random_program(prng: &mut Prng) -> Program {
    let count = prng.below(MAX_DECLARATIONS as i64 + 1) as usize;
    let mut declarations = Vec::new();
    let mut names = Vec::new();

    for _ in 0..count {
        // Declarations bind their own fresh name and may reference earlier
        // ones, matching sequential-immutable-binding semantics.
        let target = random_type(prng);
        let initializer = random_expr(prng, 3, target, &names);
        let name = declaration_name(declarations.len());
        declarations.push((name.clone(), initializer));
        names.push((name, target));
    }

    let expression = random_expr(prng, 4, random_type(prng), &names);

    Program {
        declarations,
        expression,
    }
}

fn render_program(program: &Program) -> String {
    let mut source = String::new();
    for (name, expr) in &program.declarations {
        source.push_str(&format!("let {name} = {}; ", render(expr)));
    }
    source.push_str(&render(&program.expression));
    source
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

#[test]
fn typed_expressions_match_reference_model() {
    let mut prng = Prng::new(0xA11CE);
    for _ in 0..CASES_PER_SEED {
        let target = random_type(&mut prng);
        let expr = random_expr(&mut prng, 5, target, &[]);
        let source = render(&expr);
        let expected = reference_eval(&expr, &HashMap::new());
        assert_agrees(&source, expected);
    }
}

#[test]
fn programs_match_reference_model() {
    let mut prng = Prng::new(0xBEEF);
    for _ in 0..CASES_PER_SEED {
        let program = random_program(&mut prng);
        let source = render_program(&program);
        let expected = reference_program(&program);
        assert_agrees(&source, expected);
    }
}
