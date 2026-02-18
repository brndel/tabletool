use std::sync::Arc;

use bytepack::{Pack, Unpack};
use chrono::{DateTime, Utc};
use ulid::Ulid;

use crate::{
    defs::table::TableData,
    inline_pointer::{InlinePointerPack, InlinePointerUnpack},
    named::Named,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FieldTy {
    IntI32,
    Bool,
    Timestamp,
    Text,
    RecordId { table_name: Arc<str> },
}

impl FieldTy {
    pub fn byte_count(&self) -> u32 {
        match self {
            Self::IntI32 => i32::PACK_BYTES,
            Self::Bool => bool::PACK_BYTES,
            Self::Timestamp => DateTime::<Utc>::PACK_BYTES,
            Self::Text => String::PACK_BYTES,
            Self::RecordId { .. } => Ulid::PACK_BYTES,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Ty {
    Field(FieldTy),
    Table(Named<Arc<TableData>>),
}

impl From<FieldTy> for Ty {
    fn from(value: FieldTy) -> Self {
        Ty::Field(value)
    }
}

type TagBytes = [u8; 4];

const I32_TAG: TagBytes = *b"i32 ";
const BOOL_TAG: TagBytes = *b"bool";
const TIMESTAMP_TAG: TagBytes = *b"tstp";
const TEXT_TAG: TagBytes = *b"text";
const RECORD_TAG: TagBytes = *b"rcrd";

impl Pack for FieldTy {
    const PACK_BYTES: u32 = InlinePointerPack::<FieldTy>::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        let pointer: InlinePointerPack<'_, Self> = match self {
            FieldTy::IntI32 => InlinePointerPack::Inline { tag: I32_TAG },
            FieldTy::Bool => InlinePointerPack::Inline { tag: BOOL_TAG },
            FieldTy::Timestamp => InlinePointerPack::Inline { tag: TIMESTAMP_TAG },
            FieldTy::Text => InlinePointerPack::Inline { tag: TEXT_TAG },
            FieldTy::RecordId { table_name } => InlinePointerPack::Indirect {
                tag: RECORD_TAG,
                value: table_name.as_bytes(),
            },
        };

        pointer.pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for FieldTy {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let pointer = InlinePointerUnpack::unpack(offset, unpacker)?;

        match pointer {
            InlinePointerUnpack::Inline { tag } => match tag {
                I32_TAG => Some(Self::IntI32),
                BOOL_TAG => Some(Self::Bool),
                TIMESTAMP_TAG => Some(Self::Timestamp),
                TEXT_TAG => Some(Self::Text),
                _ => None,
            },
            InlinePointerUnpack::Indirect {
                tag,
                value,
                value_offset: _,
            } => match tag {
                RECORD_TAG => {
                    let table_name = str::from_utf8(value).ok()?.into();

                    Some(Self::RecordId { table_name })
                }
                _ => None,
            },
        }
    }
}
