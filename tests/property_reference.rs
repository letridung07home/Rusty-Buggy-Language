//! A self-contained, dependency-free property-based test that checks the
//! library's evaluation against an independent reference model.
//!
//! For a deterministic number of random cases, a generator builds a
//! well-typed expression (or a program of immutable `let` bindings ending in
//! an expression) over integers, booleans, and strings, with calls to the
//! seven fixed-signature builtins (`len`, `int_to_string`, `string_to_int`,
//! `bool_to_int`, `int_to_bool`, `bool_to_string`, `string_to_bool`) and
//! lexicographic string ordering woven in
//! like any other node. It renders the tree back to source text with minimal
//! parentheses that respect the language's precedence and associativity, and
//! then requires the lexer/parser/type-checker/evaluator pipeline and a
//! straightforward recursive reference evaluator to agree: either both
//! succeed with the same [`Value`], or both fail. Being dependency-free, the
//! test runs identically on the MSRV.

use std::collections::HashMap;

use rusty_buggy_language::{evaluate, Value};

/// Max absolute value used for generated integer literals. Staying well below
/// `i64::MAX` keeps rendering unambiguous: literals print with an explicit
/// sign and the special `-9223372036854775808` case is never triggered.
const LITERAL_BOUND: i64 = 50;

/// Small strings used for generated string literals, including every escape
/// the lexer supports so the render/parse round trip is exercised. The
/// numeric texts feed `string_to_int`'s success paths directly, with leading
/// zeros and the exact `i64::MIN` boundary alongside texts (`+5`, a leading
/// space, an `i64` overflow) that both sides must reject, plus the exact
/// `true`/`false` texts `string_to_bool` accepts.
const STRING_POOL: &[&str] = &[
    "",
    "hello",
    "a b",
    "say \"hi\"",
    "tab\there",
    "new\nline",
    "back\\slash",
    "42",
    "-7",
    "007",
    "9223372036854775808",
    "-9223372036854775808",
    "true",
    "false",
];

/// The number of declaration expressions generated ahead of the final one.
const MAX_DECLARATIONS: usize = 3;

/// Percent chance that an interior expression is generated as a builtin call
/// instead of an operator node, per result type: integer-typed expressions
/// have three builtins to pick from, and string- and boolean-typed ones two
/// each. The values keep builtin calls frequent while the pre-existing
/// node kinds retain most of their original share.
const INT_BUILTIN_CHANCE: u64 = 35;
const BOOL_BUILTIN_CHANCE: u64 = 25;
const STR_BUILTIN_CHANCE: u64 = 30;

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

/// The seven fixed-signature builtin functions the language provides. The
/// generator picks them by result type and recurses with the argument type
/// the type checker demands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Builtin {
    Len,
    IntToString,
    StringToInt,
    BoolToInt,
    IntToBool,
    BoolToString,
    StringToBool,
}

impl Builtin {
    /// The identifier as it appears in source text.
    fn name(self) -> &'static str {
        match self {
            Builtin::Len => "len",
            Builtin::IntToString => "int_to_string",
            Builtin::StringToInt => "string_to_int",
            Builtin::BoolToInt => "bool_to_int",
            Builtin::IntToBool => "int_to_bool",
            Builtin::BoolToString => "bool_to_string",
            Builtin::StringToBool => "string_to_bool",
        }
    }

    /// The static type the call's single argument must have.
    fn argument_type(self) -> GenType {
        match self {
            Builtin::Len | Builtin::StringToInt | Builtin::StringToBool => GenType::Str,
            Builtin::IntToString | Builtin::IntToBool => GenType::Int,
            Builtin::BoolToInt | Builtin::BoolToString => GenType::Bool,
        }
    }
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
        builtin: Builtin,
        argument: Box<Expr>,
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
        Expr::Lit(_)
        | Expr::BoolLit(_)
        | Expr::StrLit(_)
        | Expr::Var(_)
        | Expr::Call { .. }
        | Expr::If { .. } => 6,
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
        Expr::Call { builtin, argument } => format!("{}({})", builtin.name(), render(argument)),
    }
}

/// Reference evaluation with plain typed values, mirroring the language's
/// semantics: checked `i64` arithmetic, string concatenation, the
/// fixed-signature builtins (character-count `len`, `Display`-rendered
/// `int_to_string`, the strict `string_to_int` grammar, and the `bool`/`int`
/// and `bool`/`string` conversions), short-circuiting logical operators, `if`/`else` branch
/// selection, and integer or lexicographic string ordering. Returns `None`
/// when evaluation fails; callers only compare the Ok/Err decision and, for
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
        Expr::Call { builtin, argument } => {
            let value = reference_eval(argument, values)?;
            match (builtin, value) {
                (Builtin::Len, RefValue::Str(text)) => {
                    Some(RefValue::Int(text.chars().count() as i64))
                }
                (Builtin::IntToString, RefValue::Int(value)) => {
                    Some(RefValue::Str(value.to_string()))
                }
                (Builtin::StringToInt, RefValue::Str(text)) => {
                    Some(RefValue::Int(parse_reference_integer(&text)?))
                }
                (Builtin::BoolToInt, RefValue::Bool(flag)) => Some(RefValue::Int(i64::from(flag))),
                (Builtin::IntToBool, RefValue::Int(value)) => Some(RefValue::Bool(value != 0)),
                (Builtin::BoolToString, RefValue::Bool(flag)) => {
                    let text = if flag { "true" } else { "false" };
                    Some(RefValue::Str(text.to_owned()))
                }
                (Builtin::StringToBool, RefValue::Str(text)) => match text.as_str() {
                    "true" => Some(RefValue::Bool(true)),
                    "false" => Some(RefValue::Bool(false)),
                    _ => None,
                },
                _ => None,
            }
        }
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
                BinaryOp::LessThan
                | BinaryOp::LessThanOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterThanOrEqual => {
                    // Ordering compares two integers, or two strings
                    // lexicographically (str Ord), mirroring the language;
                    // mixed pairings fail like any other type error.
                    let ordering = match (left, right) {
                        (RefValue::Int(a), RefValue::Int(b)) => a.cmp(&b),
                        (RefValue::Str(a), RefValue::Str(b)) => a.cmp(&b),
                        _ => return None,
                    };
                    Some(RefValue::Bool(match op {
                        BinaryOp::LessThan => ordering.is_lt(),
                        BinaryOp::LessThanOrEqual => ordering.is_le(),
                        BinaryOp::GreaterThan => ordering.is_gt(),
                        BinaryOp::GreaterThanOrEqual => ordering.is_ge(),
                        _ => unreachable!("handled above"),
                    }))
                }
                BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder => {
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
                        _ => unreachable!("handled above"),
                    }
                }
            }
        }
    }
}

/// Reference counterpart of `string_to_int`'s accepted grammar: an optional
/// leading `-` followed by one or more ASCII digits, with no whitespace, no
/// `+`, and a magnitude that fits into an `i64` (including the exact
/// `-9223372036854775808` boundary). Unlike the evaluator, which delegates to
/// `str::parse`, this folds the digits with checked arithmetic, so agreement
/// between the two is a real cross-check rather than a shared implementation.
fn parse_reference_integer(text: &str) -> Option<i64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // Accumulate negated so even the magnitude of `i64::MIN` fits before the
    // sign is applied; `checked_neg` then rejects the same too-large positive
    // magnitudes `parse` rejects.
    let mut accumulated: i64 = 0;
    for byte in digits.bytes() {
        let digit = i64::from(byte - b'0');
        accumulated = accumulated.checked_mul(10)?.checked_sub(digit)?;
    }
    if negative {
        Some(accumulated)
    } else {
        accumulated.checked_neg()
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
/// variables may be referenced. Interior expressions are sometimes generated
/// as builtin calls whose result type matches the target, so the builtins
/// compose with the operator, `let`, and `if` nodes like any other construct.
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
        GenType::Int => {
            if prng.chance(INT_BUILTIN_CHANCE) {
                let builtin = match prng.below(3) {
                    0 => Builtin::Len,
                    1 => Builtin::StringToInt,
                    _ => Builtin::BoolToInt,
                };
                random_builtin_call(prng, depth, builtin, names)
            } else {
                match prng.below(5) {
                    0 => Expr::Neg(Box::new(random_expr(prng, depth - 1, GenType::Int, names))),
                    1 => Expr::Binary {
                        op: int_arithmetic_operator(prng),
                        left: Box::new(random_expr(prng, depth - 1, GenType::Int, names)),
                        right: Box::new(random_expr(prng, depth - 1, GenType::Int, names)),
                    },
                    2 => random_if(prng, depth, GenType::Int, names),
                    _ => random_expr(prng, depth - 1, GenType::Int, names),
                }
            }
        }
        GenType::Bool => {
            if prng.chance(BOOL_BUILTIN_CHANCE) {
                let builtin = if prng.below(2) == 0 {
                    Builtin::IntToBool
                } else {
                    Builtin::BoolToString
                };
                random_builtin_call(prng, depth, builtin, names)
            } else {
                match prng.below(7) {
                    0 => Expr::Not(Box::new(random_expr(prng, depth - 1, GenType::Bool, names))),
                    1 => Expr::And(
                        Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
                        Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
                    ),
                    2 => Expr::Or(
                        Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
                        Box::new(random_expr(prng, depth - 1, GenType::Bool, names)),
                    ),
                    3 => random_ordering_comparison(prng, depth, names),
                    4 => random_equality(prng, depth, names),
                    5 => random_if(prng, depth, GenType::Bool, names),
                    _ => random_expr(prng, depth - 1, GenType::Bool, names),
                }
            }
        }
        GenType::Str => {
            if prng.chance(STR_BUILTIN_CHANCE) {
                let builtin = if prng.below(2) == 0 {
                    Builtin::IntToString
                } else {
                    Builtin::StringToBool
                };
                random_builtin_call(prng, depth, builtin, names)
            } else {
                match prng.below(4) {
                    0 => Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(random_expr(prng, depth - 1, GenType::Str, names)),
                        right: Box::new(random_expr(prng, depth - 1, GenType::Str, names)),
                    },
                    1 => random_if(prng, depth, GenType::Str, names),
                    _ => random_expr(prng, depth - 1, GenType::Str, names),
                }
            }
        }
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

/// A call to a fixed-signature builtin whose single argument is recursively
/// generated with the type the builtin demands, so calls nest like any other
/// node.
fn random_builtin_call(
    prng: &mut Prng,
    depth: u32,
    builtin: Builtin,
    names: &[(String, GenType)],
) -> Expr {
    Expr::Call {
        builtin,
        argument: Box::new(random_expr(prng, depth - 1, builtin.argument_type(), names)),
    }
}

/// A `<`, `<=`, `>`, or `>=` comparison of two integers or, half the time, of
/// two strings compared lexicographically, yielding a boolean.
fn random_ordering_comparison(prng: &mut Prng, depth: u32, names: &[(String, GenType)]) -> Expr {
    let op = match prng.below(4) {
        0 => BinaryOp::LessThan,
        1 => BinaryOp::LessThanOrEqual,
        2 => BinaryOp::GreaterThan,
        _ => BinaryOp::GreaterThanOrEqual,
    };
    let operand_type = if prng.chance(50) {
        GenType::Int
    } else {
        GenType::Str
    };
    Expr::Binary {
        op,
        left: Box::new(random_expr(prng, depth - 1, operand_type, names)),
        right: Box::new(random_expr(prng, depth - 1, operand_type, names)),
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

    let target = random_type(prng);
    let expression = random_expr(prng, 4, target, &names);

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

/// Pins the hand-rolled reference parser to `string_to_int`'s grammar at the
/// boundaries the random generator may not reach within a given seed.
#[test]
fn reference_integer_parser_tracks_the_builtin_grammar() {
    assert_eq!(parse_reference_integer("0"), Some(0));
    assert_eq!(parse_reference_integer("007"), Some(7));
    assert_eq!(parse_reference_integer("-7"), Some(-7));
    assert_eq!(parse_reference_integer("-0"), Some(0));
    assert_eq!(
        parse_reference_integer("9223372036854775807"),
        Some(i64::MAX)
    );
    assert_eq!(
        parse_reference_integer("-9223372036854775808"),
        Some(i64::MIN)
    );
    assert_eq!(parse_reference_integer(""), None);
    assert_eq!(parse_reference_integer("-"), None);
    assert_eq!(parse_reference_integer("+5"), None);
    assert_eq!(parse_reference_integer(" 12"), None);
    assert_eq!(parse_reference_integer("12 "), None);
    assert_eq!(parse_reference_integer("1a"), None);
    assert_eq!(parse_reference_integer("9223372036854775808"), None);
    assert_eq!(parse_reference_integer("-9223372036854775809"), None);
}

/// Pins both sides to `string_to_bool`'s grammar at the boundaries the random
/// generator may not reach within a given seed: only the exact texts `true`
/// and `false` are accepted, with no whitespace, case, or numeric tolerance.
#[test]
fn string_to_bool_boundary_texts_agree_with_the_reference() {
    for text in ["true", "false", "True", "FALSE", " true", "true ", "1", "", "tr ue"] {
        let source = format!("string_to_bool(\"{text}\")");
        let expected = match text {
            "true" => Some(RefValue::Bool(true)),
            "false" => Some(RefValue::Bool(false)),
            _ => None,
        };
        assert_agrees(&source, expected);
    }
}
