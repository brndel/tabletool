use crate::{
    ty::FieldTy,
    value::{FieldValue, Value},
};

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
                if a == &FieldTy::IntI32 && b == &FieldTy::IntI32 {
                    Some(FieldTy::IntI32)
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
                if a == &FieldTy::IntI32 && b == &FieldTy::IntI32
                    || a == &FieldTy::Timestamp && b == &FieldTy::Timestamp
                {
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

    pub fn eval(&self, a: Value, b: Value) -> Option<FieldValue> {
        let (Value::Field(a), Value::Field(b)) = (a, b) else {
            return None;
        };

        match (self, a, b) {
            (BinaryOp::Math(math_op), FieldValue::Int(a), FieldValue::Int(b)) => {
                let result = match math_op {
                    MathOp::Add => a + b,
                    MathOp::Sub => a - b,
                    MathOp::Mul => a * b,
                    MathOp::Div => a / b,
                };

                Some(FieldValue::Int(result))
            }
            (BinaryOp::Math(_), _, _) => None,
            (BinaryOp::Logic(logic_op), FieldValue::Bool(a), FieldValue::Bool(b)) => {
                let result = match logic_op {
                    LogicOp::And => a && b,
                    LogicOp::Or => a || b,
                };

                Some(FieldValue::Bool(result))
            }
            (BinaryOp::Logic(_), _, _) => None,
            (BinaryOp::Compare(compare_op), FieldValue::Int(a), FieldValue::Int(b)) => {
                let result = match compare_op {
                    CompareOp::Less => a < b,
                    CompareOp::LessEq => a <= b,
                    CompareOp::Greater => a > b,
                    CompareOp::GreaterEq => a >= b,
                };
                Some(FieldValue::Bool(result))
            }
            (BinaryOp::Compare(compare_op), FieldValue::Timestamp(a), FieldValue::Timestamp(b)) => {
                let result = match compare_op {
                    CompareOp::Less => a < b,
                    CompareOp::LessEq => a <= b,
                    CompareOp::Greater => a > b,
                    CompareOp::GreaterEq => a >= b,
                };
                Some(FieldValue::Bool(result))
            }
            (BinaryOp::Compare(_), _, _) => None,
            (BinaryOp::Eq(eq_op), FieldValue::Int(a), FieldValue::Int(b)) => {
                Some(FieldValue::Bool(eq_op.eval(&a, &b)))
            }
            (BinaryOp::Eq(eq_op), FieldValue::Bool(a), FieldValue::Bool(b)) => {
                Some(FieldValue::Bool(eq_op.eval(&a, &b)))
            }
            (BinaryOp::Eq(eq_op), FieldValue::Timestamp(a), FieldValue::Timestamp(b)) => {
                Some(FieldValue::Bool(eq_op.eval(&a, &b)))
            }
            (BinaryOp::Eq(eq_op), FieldValue::Text(a), FieldValue::Text(b)) => {
                Some(FieldValue::Bool(eq_op.eval(&a, &b)))
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
                if value == &FieldTy::IntI32 {
                    Some(FieldTy::IntI32)
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
        let Value::Field(value) = value else {
            return None;
        };

        match (self, value) {
            (UnaryOp::Negate, FieldValue::Int(value)) => Some(FieldValue::Int(-value).into()),
            (UnaryOp::LogicNot, FieldValue::Bool(value)) => Some(FieldValue::Bool(!value).into()),
            _ => None,
        }
    }
}
