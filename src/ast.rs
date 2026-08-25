use crate::error::SourcePosition;

#[derive(Debug, PartialEq)]
pub(crate) enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
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
    Variable {
        name: String,
        position: Option<SourcePosition>,
    },
    UnaryNegation {
        operand: Box<Expression>,
        position: Option<SourcePosition>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
        position: Option<SourcePosition>,
    },
}

impl Expression {
    /// The position of the first token that begins this expression, if known.
    pub(crate) fn position(&self) -> Option<SourcePosition> {
        match self {
            Expression::Literal { position, .. }
            | Expression::Variable { position, .. }
            | Expression::UnaryNegation { position, .. }
            | Expression::Binary { position, .. } => *position,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) initializer: Expression,
    pub(crate) position: Option<SourcePosition>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Program {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) expression: Expression,
}
