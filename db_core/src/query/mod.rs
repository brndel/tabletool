use std::sync::Arc;

use crate::expr::Expr;


#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub table_name: Arc<str>,
    pub filter: Option<Expr>,
}

