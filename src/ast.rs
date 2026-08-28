use crate::error::SourcePosition;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BinaryOperator {
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

#[derive(Debug, PartialEq)]
pub(crate) enum Expression {
    Literal {
        value: u64,
        position: Option<SourcePosition>,
    },
    StringLiteral {
        value: String,
        position: Option<SourcePosition>,
    },
    BoolLiteral {
        value: bool,
        position: Option<SourcePosition>,
    },
    Variable {
        name: String,
        position: Option<SourcePosition>,
    },
    Call {
        callee: String,
        arguments: Vec<Expression>,
        position: Option<SourcePosition>,
    },
    UnaryNegation {
        operand: Box<Expression>,
        position: Option<SourcePosition>,
    },
    UnaryNot {
        operand: Box<Expression>,
        position: Option<SourcePosition>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
        position: Option<SourcePosition>,
    },
    LogicalAnd {
        left: Box<Expression>,
        right: Box<Expression>,
        position: Option<SourcePosition>,
    },
    LogicalOr {
        left: Box<Expression>,
        right: Box<Expression>,
        position: Option<SourcePosition>,
    },
    If {
        condition: Box<Expression>,
        then_branch: Box<Block>,
        else_branch: Box<Block>,
        position: Option<SourcePosition>,
    },
}

impl Expression {
    /// The position of the first token that begins this expression, if known.
    pub(crate) fn position(&self) -> Option<SourcePosition> {
        match self {
            Expression::Literal { position, .. }
            | Expression::StringLiteral { position, .. }
            | Expression::BoolLiteral { position, .. }
            | Expression::Variable { position, .. }
            | Expression::Call { position, .. }
            | Expression::UnaryNegation { position, .. }
            | Expression::UnaryNot { position, .. }
            | Expression::Binary { position, .. }
            | Expression::LogicalAnd { position, .. }
            | Expression::LogicalOr { position, .. }
            | Expression::If { position, .. } => *position,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) initializer: Expression,
    pub(crate) position: Option<SourcePosition>,
}

/// A `{ declaration* expression }` block, used as the branch of an `if`/`else`
/// expression and as a function body. Declarations inside a block are scoped
/// to that block.
#[derive(Debug, PartialEq)]
pub(crate) struct Block {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) expression: Expression,
}

/// A `fn name(param, ...) = <body>;` declaration. Parameters are immutable and
/// scoped to the body; the body is a block (`{ declaration* expression }`).
/// Functions are visible to later declarations, the final expression, and to
/// recursive calls from their own body.
#[derive(Debug, PartialEq)]
pub(crate) struct FunctionDeclaration {
    pub(crate) name: String,
    pub(crate) parameters: Vec<String>,
    pub(crate) body: Block,
    pub(crate) position: Option<SourcePosition>,
}

/// A resolved function binding: the named declaration plus the resolved
/// parameter types and result type from monomorphic inference. Shared between
/// the type checker and the evaluator.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) parameters: Vec<String>,
    pub(crate) parameter_types: Vec<Type>,
    pub(crate) result_type: Type,
    pub(crate) body: Block,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Program {
    pub(crate) functions: Vec<FunctionDeclaration>,
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) expression: Expression,
}

/// The three value types of the language, used both by the static type checker
/// and as resolved function signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Type {
    Int,
    Bool,
    String,
}

impl Type {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Type::Int => "integer",
            Type::Bool => "boolean",
            Type::String => "string",
        }
    }
}
