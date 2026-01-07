use std::sync::Arc;

use ulid::Ulid;

use crate::FieldType;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("{0}")]
    Redb(redb::Error),
    #[error("Record {table}:{record} does not exist")]
    RecordDoesNotExist { table: Arc<str>, record: Ulid },
    #[error("Expected type '{expected:?}'")]
    WrongType { expected: FieldType },
    #[error("Table {table} does not exist")]
    TableDoesNotExist { table: Arc<str> },
}

impl<T: Into<redb::Error>> From<T> for DbError {
    fn from(value: T) -> Self {
        Self::Redb(value.into())
    }
}
