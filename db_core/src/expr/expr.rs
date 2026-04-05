use std::sync::Arc;

use chumsky::span::SimpleSpan;

use crate::{
    expr::{
        Spanned,
        op::{BinaryOp, UnaryOp},
    },
    named::Named,
    value::FieldValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Literal(Spanned<FieldValue>),
    Array(Vec<Spanned<Self>>),
    Struct {
        fields: Vec<Spanned<Named<Self>>>,
    },
    BinaryOp {
        a: Spanned<Box<Self>>,
        op: Spanned<BinaryOp>,
        b: Spanned<Box<Self>>,
    },
    UnaryOp {
        op: Spanned<UnaryOp>,
        value: Spanned<Box<Self>>,
    },
    FieldAccess {
        value: Spanned<Box<Self>>,
        dot_span: SimpleSpan,
        field: Option<Spanned<Arc<str>>>,
    },
    Variable {
        name: Spanned<Arc<str>>,
    },
    FnCall {
        name: Spanned<Arc<str>>,
        args: Vec<Spanned<Self>>,
    },
    LambdaFn {
        args: Vec<Spanned<Arc<str>>>,
        body: Spanned<Box<Self>>
    },
    /// Used for autocompletion slots
    EmptyPlaceholder,
}
