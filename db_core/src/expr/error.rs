use std::{fmt::Display, sync::Arc};

use crate::{expr::{BinaryOp, UnaryOp}, ty::Ty};

#[derive(Debug, thiserror::Error)]
pub enum EvalErr {
    #[error("Missmatched types: found {found:?}, expected {expected:?}")]
    MissmatchedTypes { found: Ty, expected: Option<Ty> },
    #[error("Binary op '{op:?}' does not work with types a:{a:?}, b:{b:?}")]
    InvalidTypeForBinaryOp { op: BinaryOp, a: Ty, b: Ty },
    #[error("Unary op '{op:?}' does not work with type {ty:?}")]
    InvalidTypeForUnaryOp { op: UnaryOp, ty: Ty },
    #[error("Unknown Table '{name}'{did_you_mean_hint}")]
    UnknownTable {
        name: Arc<str>,
        did_you_mean_hint: DidYouMeanHint,
    },
    #[error("Unknown Field '{name}' on table '{table_name}'")]
    UnknownField {
        name: Arc<str>,
        table_name: Arc<str>,
    },
    #[error("Unknown Function '{name}'")]
    UnknownFunction { name: Arc<str> },
    #[error("Invalid arg count for function '{name}': found: {found}, expected: {expected}")]
    InvalidFunctionArgCount { name: Arc<str>, found: usize, expected: usize },
    #[error("Bytepack Error")]
    Bytepack,
}

#[derive(Debug, Clone, Default)]
pub enum DidYouMeanHint {
    #[default]
    None,
    Table {
        name: Arc<str>,
    },
    TableWithField {
        table_name: Arc<str>,
        field_name: Arc<str>,
    },
}

impl Display for DidYouMeanHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DidYouMeanHint::None => Ok(()),
            DidYouMeanHint::Table { name } => write!(f, " Did you mean '{name}'"),
            DidYouMeanHint::TableWithField {
                table_name,
                field_name,
            } => write!(f, " Did you mean '{table_name}.{field_name}'"),
        }
    }
}
