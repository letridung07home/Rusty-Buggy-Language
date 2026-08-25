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
    Literal(u64),
    Variable(String),
    UnaryNegation(Box<Expression>),
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

#[derive(Debug, PartialEq)]
pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) initializer: Expression,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Program {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) expression: Expression,
}
