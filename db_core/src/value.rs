use std::sync::Arc;

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
    Record {
        table: Named<Arc<TableData>>,
        record: Arc<RecordBytes>,
    },
}

impl Value {
    pub fn ty(&self) -> Ty {
        match self {
            Value::Int(_) => Ty::Field(FieldTy::Int),
            Value::Bool(_) => Ty::Field(FieldTy::Bool),
            Value::Record { table, .. } => Ty::Table(table.clone()),
        }
    }
}
