mod result;

use std::sync::Arc;

use crate::expr::{Expr, Spanned};

pub use result::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub table_name: Spanned<Arc<str>>,
    pub filter: Option<Spanned<Expr>>,
    pub group_by: Option<Spanned<Expr>>,
    pub group_extra: Option<Spanned<Expr>>,
}

