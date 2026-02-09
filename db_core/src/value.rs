use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::{
    defs::table::TableData,
    named::Named,
    record::RecordBytes,
    ty::{FieldTy, Ty},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    Int(i32),
    Bool(bool),
    DateTime(DateTime<Utc>),
    Text(String),
    Record {
        table: Named<Arc<TableData>>,
        record: Arc<RecordBytes>,
    },
}

impl Value {
    pub fn ty(&self) -> Ty {
        match self {
            Value::Int(_) => Ty::Field(FieldTy::IntI32),
            Value::Bool(_) => Ty::Field(FieldTy::Bool),
            Value::DateTime(_) => Ty::Field(FieldTy::Timestamp),
            Value::Text(_) => Ty::Field(FieldTy::Text),
            Value::Record { table, .. } => Ty::Table(table.clone()),
        }
    }
}
