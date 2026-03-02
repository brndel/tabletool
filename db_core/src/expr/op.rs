#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    LogicNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Math(MathOp),
    Logic(LogicOp),
    Compare(CompareOp),
    Eq(EqOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Less,
    LessEq,
    Greater,
    GreaterEq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqOp {
    Eq,
    Neq,
}
