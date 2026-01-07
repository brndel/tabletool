use crate::{ty::FieldTy, value::Value};

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

impl BinaryOp {
    pub fn ty(&self, a: &FieldTy, b: &FieldTy) -> Option<FieldTy> {
        match self {
            BinaryOp::Math(_) => {
                if a == &FieldTy::Int && b == &FieldTy::Int {
                    Some(FieldTy::Int)
                } else {
                    None
                }
            }
            BinaryOp::Logic(_) => {
                if a == &FieldTy::Bool && b == &FieldTy::Bool {
                    Some(FieldTy::Bool)
                } else {
                    None
                }
            }
            BinaryOp::Compare(_) => {
                if a == &FieldTy::Int && b == &FieldTy::Int {
                    Some(FieldTy::Bool)
                } else {
                    None
                }
            }
            BinaryOp::Eq(_) => {
                if a == b {
                    Some(FieldTy::Bool)
                } else {
                    None
                }
            }
        }
    }

    pub fn eval(&self, a: Value, b: Value) -> Option<Value> {
        match (self, a, b) {
            (BinaryOp::Math(math_op), Value::Int(a), Value::Int(b)) => {
                let result = match math_op {
                    MathOp::Add => a + b,
                    MathOp::Sub => a - b,
                    MathOp::Mul => a * b,
                    MathOp::Div => a / b,
                };

                Some(Value::Int(result))
            }
            (BinaryOp::Math(_), _, _) => None,
            (BinaryOp::Logic(logic_op), Value::Bool(a), Value::Bool(b)) => {
                let result = match logic_op {
                    LogicOp::And => a && b,
                    LogicOp::Or => a || b,
                };

                Some(Value::Bool(result))
            }
            (BinaryOp::Logic(_), _, _) => None,
            (BinaryOp::Compare(compare_op), Value::Int(a), Value::Int(b)) => {
                let result = match compare_op {
                    CompareOp::Less => a < b,
                    CompareOp::LessEq => a <= b,
                    CompareOp::Greater => a > b,
                    CompareOp::GreaterEq => a >= b,
                };
                Some(Value::Bool(result))
            }
            (BinaryOp::Compare(_), _, _) => None,
            (BinaryOp::Eq(eq_op), Value::Int(a), Value::Int(b)) => {
                Some(Value::Bool(eq_op.eval(&a, &b)))
            }
            (BinaryOp::Eq(eq_op), Value::Bool(a), Value::Bool(b)) => {
                Some(Value::Bool(eq_op.eval(&a, &b)))
            }
            (BinaryOp::Eq(_), _, _) => None,
        }
    }
}

impl EqOp {
    fn eval<T: PartialEq>(&self, a: &T, b: &T) -> bool {
        match self {
            EqOp::Eq => a == b,
            EqOp::Neq => a != b,
        }
    }
}

impl UnaryOp {
    pub fn ty(&self, value: &FieldTy) -> Option<FieldTy> {
        match self {
            UnaryOp::Negate => {
                if value == &FieldTy::Int {
                    Some(FieldTy::Int)
                } else {
                    None
                }
            }
            UnaryOp::LogicNot => {
                if value == &FieldTy::Bool {
                    Some(FieldTy::Bool)
                } else {
                    None
                }
            }
        }
    }

    pub fn eval(&self, value: Value) -> Option<Value> {
        match (self, value) {
            (UnaryOp::Negate, Value::Int(value)) => Some(Value::Int(-value)),
            (UnaryOp::LogicNot, Value::Bool(value)) => Some(Value::Bool(!value)),
            _ => None,
        }
    }
}
