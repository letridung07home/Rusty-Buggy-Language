//! A self-contained, dependency-free property-based test that checks the
//! library's evaluation against an independent reference model.
//!
//! For a deterministic number of random cases, a generator builds an
//! expression (or a program of immutable `let` bindings ending in an
//! expression), renders it back to source text with minimal parentheses that
//! respect the language's precedence and associativity, and then requires the
//! lexer/parser/evaluator pipeline and a straightforward recursive reference
//! evaluator to agree: either both succeed with the same integer, or both
//! fail. Being dependency-free, the test runs identically on the MSRV.

use std::collections::HashMap;

use rusty_buggy_language::evaluate;

/// Max absolute value used for generated integer literals. Staying well below
/// `i64::MAX` keeps rendering unambiguous: literals print with an explicit
/// sign and the special `-9223372036854775808` case is never triggered.
const LITERAL_BOUND: i64 = 50;

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

#[derive(Debug, Clone, PartialEq)]
enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    LessThan,
    GreaterThan,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Lit(i64),
    Var(String),
    Neg(Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
struct Program {
    declarations: Vec<(String, Expr)>,
    expression: Expr,
}

/// Precedence used both by the reference model and for minimal-paren
/// rendering. Higher binds tighter. Comparisons are the lowest level so
/// programs never chain or mix them.
fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::Lit(_) | Expr::Var(_) => 5,
        Expr::Neg(_) => 4,
        Expr::Binary { op, .. } => match op {
            BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder => 3,
            BinaryOp::Add | BinaryOp::Subtract => 2,
            BinaryOp::LessThan | BinaryOp::GreaterThan => 1,
        },
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

/// Renders an expression with the minimum parentheses needed to preserve its
/// tree under the language's precedence and associativity rules. Binary
/// operators are left-associative; negation is right-associative.
fn render(expr: &Expr) -> String {
    match expr {
        Expr::Lit(value) => value.to_string(),
        Expr::Var(name) => name.clone(),
        Expr::Neg(operand) => format!("-{}", wrap(operand, 4)),
        Expr::Binary { op, left, right } => {
            let operator = match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                BinaryOp::Remainder => "%",
                BinaryOp::LessThan => "<",
                BinaryOp::GreaterThan => ">",
            };
            let this_precedence = precedence(expr);
            // Binary operators are left-associative, but the language allows at
            // most one comparison per expression level. A nested comparison is
            // therefore parenthesized on both sides so it is never rendered as
            // an illegal chain such as `9 < 3 > 1`.
            let comparison = matches!(*op, BinaryOp::LessThan | BinaryOp::GreaterThan);
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
    }
}

/// Reference evaluation with plain `i64` checked arithmetic, mirroring the
/// language's semantics. Returns `None` when evaluation fails; callers only
/// compare the Ok/Err decision and, for successes, the value.
fn reference_eval(expr: &Expr, values: &HashMap<String, i64>) -> Option<i64> {
    match expr {
        Expr::Lit(value) => Some(*value),
        Expr::Var(name) => values.get(name).copied(),
        Expr::Neg(operand) => reference_eval(operand, values)?.checked_neg(),
        Expr::Binary { op, left, right } => {
            let left = reference_eval(left, values)?;
            let right = reference_eval(right, values)?;
            match op {
                BinaryOp::Add => left.checked_add(right),
                BinaryOp::Subtract => left.checked_sub(right),
                BinaryOp::Multiply => left.checked_mul(right),
                BinaryOp::Divide => {
                    if right == 0 {
                        None
                    } else {
                        left.checked_div(right)
                    }
                }
                BinaryOp::Remainder => {
                    if right == 0 {
                        None
                    } else {
                        left.checked_rem(right)
                    }
                }
                BinaryOp::LessThan => Some(if left < right { 1 } else { 0 }),
                BinaryOp::GreaterThan => Some(if left > right { 1 } else { 0 }),
            }
        }
    }
}

fn reference_program(program: &Program) -> Option<i64> {
    let mut values = HashMap::new();
    for (name, expr) in &program.declarations {
        values.insert(name.clone(), reference_eval(expr, &values)?);
    }
    reference_eval(&program.expression, &values)
}

/// Generates a random expression of the given target depth. `names` are the
/// identifiers a variable leaf may reference (empty for pure literals).
fn random_expr(prng: &mut Prng, depth: u32, names: &[String]) -> Expr {
    if depth == 0 || prng.chance(35) {
        // Leaf: a literal, or a reference to a declared variable when one
        // exists.
        if !names.is_empty() && prng.chance(40) {
            let index = prng.below(names.len() as i64) as usize;
            return Expr::Var(names[index].clone());
        }
        return Expr::Lit(prng.below(LITERAL_BOUND * 2 + 1) - LITERAL_BOUND);
    }

    match prng.below(3) {
        0 => Expr::Neg(Box::new(random_expr(prng, depth - 1, names))),
        1 => {
            let left = random_expr(prng, depth - 1, names);
            let right = random_expr(prng, depth - 1, names);
            Expr::Binary {
                op: random_operator(prng),
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        _ => random_expr(prng, depth - 1, names),
    }
}

fn random_operator(prng: &mut Prng) -> BinaryOp {
    match prng.below(7) {
        0 => BinaryOp::Add,
        1 => BinaryOp::Subtract,
        2 => BinaryOp::Multiply,
        3 => BinaryOp::Divide,
        4 => BinaryOp::Remainder,
        5 => BinaryOp::LessThan,
        _ => BinaryOp::GreaterThan,
    }
}

fn declaration_name(index: usize) -> String {
    format!("value{index}")
}

fn random_program(prng: &mut Prng) -> Program {
    let count = prng.below(MAX_DECLARATIONS as i64 + 1) as usize;
    let mut declarations = Vec::new();
    for _ in 0..count {
        // Declarations bind their own fresh name and never reference later
        // ones, matching sequential-immutable-binding semantics.
        let initializer = random_expr(prng, 3, &[]);
        declarations.push((declaration_name(declarations.len()), initializer));
    }

    let names: Vec<String> = declarations.iter().map(|(name, _)| name.clone()).collect();
    let expression = random_expr(prng, 4, &names);

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
fn assert_agrees(source: &str, expected: Option<i64>) {
    match (evaluate(source), expected) {
        (Ok(actual), Some(expect)) => assert_eq!(actual, expect, "source: {source}"),
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
fn arithmetic_matches_reference_model() {
    let mut prng = Prng::new(0xA11CE);
    for _ in 0..CASES_PER_SEED {
        let expr = random_expr(&mut prng, 5, &[]);
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
