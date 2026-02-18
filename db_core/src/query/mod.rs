mod result;

use std::sync::Arc;

use crate::expr::Expr;

pub use result::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub table_name: Arc<str>,
    pub filter: Option<Expr>,
    pub group_by: Option<Expr>,
}

