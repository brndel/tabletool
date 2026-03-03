use std::sync::Arc;

use crate::{
    expr::
        op::{BinaryOp, UnaryOp}
    ,
    value::FieldValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Literal(FieldValue),
    Array(Vec<Self>),
    BinaryOp {
        a: Box<Self>,
        op: BinaryOp,
        b: Box<Self>,
    },
    UnaryOp {
        op: UnaryOp,
        value: Box<Self>,
    },
    FieldAccess {
        value: Box<Self>,
        field: Arc<str>,
    },
    TableAccess {
        name: Arc<str>,
    },
    FnCall {
        name: Arc<str>,
        args: Vec<Self>,
    },
}