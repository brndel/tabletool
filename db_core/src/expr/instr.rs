use std::sync::Arc;

use chumsky::span::SimpleSpan;

use crate::{
    expr::{Expr, Spanned},
    ty::Ty,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Let {
        let_span: SimpleSpan,
        name: Spanned<Arc<str>>,
        ty: Option<Spanned<Ty>>,
        expr: Spanned<Expr>,
    },
    Return {
        return_span: SimpleSpan,
        expr: Spanned<Expr>,
    },
    Expr {
        expr: Spanned<Expr>,
    },
}
