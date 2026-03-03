use std::sync::Arc;

use db_core::{asm_code::AsmCompileErr, ty::FieldTy};
use ulid::Ulid;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("{0}")]
    Redb(redb::Error),
    #[error("Record {table}:{record} does not exist")]
    RecordDoesNotExist { table: Arc<str>, record: Ulid },
    #[error("Expected type '{expected:?}'")]
    WrongType { expected: FieldTy },
    #[error("Table {table} does not exist")]
    TableDoesNotExist { table: Arc<str> },
    #[error("Expression compile error\n{0}")]
    ExprCompileError(AsmCompileErr),
    #[error("Expression error {0}")]
    ExprError(&'static str),
    #[error("Expression did panic")]
    ExprPanic(String),
}

impl<T: Into<redb::Error>> From<T> for DbError {
    fn from(value: T) -> Self {
        Self::Redb(value.into())
    }
}
