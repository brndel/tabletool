use std::sync::Arc;

use bytepack::Pack;
use chrono::{DateTime, Utc};
use ulid::Ulid;

use crate::{
    defs::table::TableData,
    named::Named,
    record::RecordBytes,
    ty::{FieldTy, Ty},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FieldValue {
    Int(i32),
    Bool(bool),
    Timestamp(DateTime<Utc>),
    Text(String),
    RecordId { id: Ulid, table_name: Arc<str> },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    Field(FieldValue),
    Record {
        table: Named<Arc<TableData>>,
        record: Arc<RecordBytes>,
    },
}

impl FieldValue {
    pub fn ty(&self) -> FieldTy {
        match self {
            FieldValue::Int(_) => FieldTy::IntI32,
            FieldValue::Bool(_) => FieldTy::Bool,
            FieldValue::Timestamp(_) => FieldTy::Timestamp,
            FieldValue::Text(_) => FieldTy::Text,
            Self::RecordId { table_name, .. } => FieldTy::RecordId {
                table_name: table_name.clone(),
            },
        }
    }

    pub fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        match self {
            FieldValue::Int(value) => value.pack(offset, packer),
            FieldValue::Bool(value) => value.pack(offset, packer),
            FieldValue::Timestamp(value) => value.pack(offset, packer),
            FieldValue::Text(value) => value.pack(offset, packer),
            FieldValue::RecordId {
                id: value,
                table_name: _,
            } => value.pack(offset, packer),
        }
    }
}

impl Value {
    pub fn ty(&self) -> Ty {
        match self {
            Value::Field(field_value) => Ty::Field(field_value.ty()),
            Value::Record { table, record: _ } => Ty::Table(table.clone()),
        }
    }
}

impl From<FieldValue> for Value {
    fn from(value: FieldValue) -> Self {
        Self::Field(value)
    }
}
