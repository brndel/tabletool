use bytepack::Pack;
use chrono::{DateTime, Utc};
use ulid::Ulid;

#[derive(Debug, Clone)]
pub enum FieldValue {
    Text(String),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Bool(bool),
    DateTime(DateTime<Utc>),
    RecordId(Ulid),
}

impl Pack for FieldValue {
    const PACK_BYTES: u32 = 0;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        match self {
            FieldValue::Text(value) => value.pack(offset, packer),
            FieldValue::U8(value) => value.pack(offset, packer),
            FieldValue::U16(value) => value.pack(offset, packer),
            FieldValue::U32(value) => value.pack(offset, packer),
            FieldValue::U64(value) => value.pack(offset, packer),
            FieldValue::U128(value) => value.pack(offset, packer),
            FieldValue::I8(value) => value.pack(offset, packer),
            FieldValue::I16(value) => value.pack(offset, packer),
            FieldValue::I32(value) => value.pack(offset, packer),
            FieldValue::I64(value) => value.pack(offset, packer),
            FieldValue::I128(value) => value.pack(offset, packer),
            FieldValue::Bool(value) => value.pack(offset, packer),
            FieldValue::DateTime(value) => value.pack(offset, packer),
            FieldValue::RecordId(value) => value.pack(offset, packer),
        }
    }
}
