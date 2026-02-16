use bytepack::{ByteUnpacker, Unpack};
use ulid::Ulid;

use crate::{defs::table::TableFieldData, ty::FieldTy, value::FieldValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBytes {
    id: Ulid,
    bytes: Vec<u8>,
}

impl RecordBytes {
    pub fn new(id: Ulid, bytes: Vec<u8>) -> Self {
        Self { id, bytes }
    }

    pub fn create(bytes: Vec<u8>) -> Self {
        Self {
            id: Ulid::new(),
            bytes,
        }
    }

    pub fn id(&self) -> Ulid {
        self.id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn get_field(&self, field: &TableFieldData) -> Option<FieldValue> {
        let value = match &field.ty {
            FieldTy::IntI32 => FieldValue::Int(self.unpack(field.offset)?),
            FieldTy::Bool => FieldValue::Bool(self.unpack(field.offset)?),
            FieldTy::Timestamp => FieldValue::Timestamp(self.unpack(field.offset)?),
            FieldTy::Text => FieldValue::Text(self.unpack(field.offset)?),
            FieldTy::RecordId { table_name } => FieldValue::RecordId { id: self.unpack(field.offset)?, table_name: table_name.clone() },
        };

        Some(value)
    }

    pub fn unpack<'a, T: Unpack<'a>>(&'a self, offset: u32) -> Option<T> {
        T::unpack(offset, &ByteUnpacker::new(self.bytes()))
    }
}
