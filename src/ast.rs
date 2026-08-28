use crate::error::SourcePosition;

#[derive(Debug, PartialEq)]
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
        then_branch: Block,
        else_branch: Block,
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
/// expression. Declarations inside a block are scoped to that block.
#[derive(Debug, PartialEq)]
pub(crate) struct Block {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) expression: Expression,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Program {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) expression: Expression,
}
