use std::sync::Arc;

use chumsky::span::SimpleSpan;

use crate::{
    expr::{
        Spanned,
        instr::Instruction,
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
        a: Box<Spanned<Self>>,
        op: Spanned<BinaryOp>,
        b: Box<Spanned<Self>>,
    },
    UnaryOp {
        op: Spanned<UnaryOp>,
        value: Box<Spanned<Self>>,
    },
    FieldAccess {
        value: Box<Spanned<Self>>,
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
        body: Box<Spanned<Self>>,
    },
    Block(ExprBlock),
    Query(QueryExpr),
    /// Used for autocompletion slots
    EmptyPlaceholder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprBlock {
    pub instructions: Vec<Spanned<Instruction>>,
    pub return_expr: Option<Box<Spanned<Expr>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExpr {
    pub table_name: Spanned<Arc<str>>,
    pub filter: Option<Box<Spanned<Expr>>>,
}
