use std::{fmt::Display, sync::Arc};

use chrono::DateTime;
use ulid::Ulid;

use bytepack::{Pack, PackPointer, Unpack};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Number(NumberFieldType),
    DateTime,
    Text,
    Record { table_name: Arc<str> },
    List { ty: Box<Self> },
    Option { ty: Box<Self> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberFieldType {
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
}

impl Pack for FieldType {
    const PACK_BYTES: u32 = MaybeInlinePointerValue::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        self.pointer_value().pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for FieldType {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let pointer_value = MaybeInlinePointerValueUnpack::unpack(offset, unpacker)?;
        match pointer_value {
            MaybeInlinePointerValueUnpack::Inline { tag } => match &tag {
                b"text" => Some(Self::Text),
                b"dati" => Some(Self::DateTime),
                b"u8  " => Some(Self::Number(NumberFieldType::U8)),
                b"u16 " => Some(Self::Number(NumberFieldType::U16)),
                b"u32 " => Some(Self::Number(NumberFieldType::U32)),
                b"u64 " => Some(Self::Number(NumberFieldType::U64)),
                b"u128" => Some(Self::Number(NumberFieldType::U128)),
                b"i8  " => Some(Self::Number(NumberFieldType::I8)),
                b"i16 " => Some(Self::Number(NumberFieldType::I16)),
                b"i32 " => Some(Self::Number(NumberFieldType::I32)),
                b"i64 " => Some(Self::Number(NumberFieldType::I64)),
                b"i128" => Some(Self::Number(NumberFieldType::I128)),
                _ => None,
            },
            MaybeInlinePointerValueUnpack::Indirect {
                tag,
                value,
                value_offset,
            } => match &tag {
                b"rcrd" => {
                    let table_name = str::from_utf8(value).ok()?.into();

                    Some(Self::Record { table_name })
                }
                b"list" => {
                    let inner_ty = FieldType::unpack(value_offset, unpacker)?;

                    Some(Self::List {
                        ty: Box::new(inner_ty),
                    })
                }
                b"opti" => {
                    let inner_ty = FieldType::unpack(value_offset, unpacker)?;

                    Some(Self::Option {
                        ty: Box::new(inner_ty),
                    })
                }
                _ => None,
            },
        }
    }
}

enum MaybeInlinePointerValue<'a> {
    Inline { tag: [u8; 4] },
    Indirect { tag: [u8; 4], value: &'a [u8] },
    Nested { tag: [u8; 4], value: &'a FieldType },
}

impl<'a> Pack for MaybeInlinePointerValue<'a> {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        let pointer = match self {
            MaybeInlinePointerValue::Inline { tag } => PackPointer {
                offset: 0,
                len: u32::from_be_bytes(*tag),
            },
            MaybeInlinePointerValue::Indirect { tag, value } => {
                let tag_pointer = packer.push_dynamic(tag);
                let value_pointer = packer.push_dynamic(value);

                PackPointer {
                    offset: tag_pointer.offset,
                    len: tag_pointer.len + value_pointer.len,
                }
            }
            MaybeInlinePointerValue::Nested { tag, value } => {
                let tag_pointer = packer.push_dynamic(tag);
                let value_pointer = packer.push_dynamic(&[0; FieldType::PACK_BYTES as usize]);

                value.pack(value_pointer.offset, packer);

                PackPointer {
                    offset: tag_pointer.offset,
                    len: tag_pointer.len + value_pointer.len,
                }
            }
        };

        pointer.pack(offset, packer);
    }
}

enum MaybeInlinePointerValueUnpack<'a> {
    Inline {
        tag: [u8; 4],
    },
    Indirect {
        tag: [u8; 4],
        value: &'a [u8],
        value_offset: u32,
    },
}

impl<'b> Unpack<'b> for MaybeInlinePointerValueUnpack<'b> {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let pointer = PackPointer::unpack(offset, unpacker)?;

        if pointer.offset == 0 {
            let tag = pointer.len.to_be_bytes();
            Some(Self::Inline { tag })
        } else {
            let value = unpacker.read_bytes(pointer);

            let tag = value[0..4].try_into().ok()?;
            let value = &value[4..];

            Some(Self::Indirect {
                tag,
                value,
                value_offset: pointer.offset + 4,
            })
        }
    }
}

impl FieldType {
    fn pointer_value(&self) -> MaybeInlinePointerValue<'_> {
        match self {
            FieldType::Record { table_name } => MaybeInlinePointerValue::Indirect {
                tag: *b"rcrd",
                value: table_name.as_bytes(),
            },
            FieldType::List { ty } => MaybeInlinePointerValue::Nested {
                tag: *b"list",
                value: ty,
            },
            FieldType::Option { ty } => MaybeInlinePointerValue::Nested {
                tag: *b"opti",
                value: ty,
            },
            FieldType::Text => MaybeInlinePointerValue::Inline { tag: *b"text" },
            FieldType::DateTime => MaybeInlinePointerValue::Inline { tag: *b"dati" },
            FieldType::Number(NumberFieldType::U8) => {
                MaybeInlinePointerValue::Inline { tag: *b"u8  " }
            }
            FieldType::Number(NumberFieldType::U16) => {
                MaybeInlinePointerValue::Inline { tag: *b"u16 " }
            }
            FieldType::Number(NumberFieldType::U32) => {
                MaybeInlinePointerValue::Inline { tag: *b"u32 " }
            }
            FieldType::Number(NumberFieldType::U64) => {
                MaybeInlinePointerValue::Inline { tag: *b"u64 " }
            }
            FieldType::Number(NumberFieldType::U128) => {
                MaybeInlinePointerValue::Inline { tag: *b"u128" }
            }
            FieldType::Number(NumberFieldType::I8) => {
                MaybeInlinePointerValue::Inline { tag: *b"i8  " }
            }
            FieldType::Number(NumberFieldType::I16) => {
                MaybeInlinePointerValue::Inline { tag: *b"i16 " }
            }
            FieldType::Number(NumberFieldType::I32) => {
                MaybeInlinePointerValue::Inline { tag: *b"i32 " }
            }
            FieldType::Number(NumberFieldType::I64) => {
                MaybeInlinePointerValue::Inline { tag: *b"i64 " }
            }
            FieldType::Number(NumberFieldType::I128) => {
                MaybeInlinePointerValue::Inline { tag: *b"i128" }
            }
        }
    }

    pub fn byte_count(&self) -> u32 {
        match self {
            FieldType::Number(NumberFieldType::U8) => u8::PACK_BYTES,
            FieldType::Number(NumberFieldType::U16) => u16::PACK_BYTES,
            FieldType::Number(NumberFieldType::U32) => u32::PACK_BYTES,
            FieldType::Number(NumberFieldType::U64) => u64::PACK_BYTES,
            FieldType::Number(NumberFieldType::U128) => u128::PACK_BYTES,
            FieldType::Number(NumberFieldType::I8) => i8::PACK_BYTES,
            FieldType::Number(NumberFieldType::I16) => i16::PACK_BYTES,
            FieldType::Number(NumberFieldType::I32) => i32::PACK_BYTES,
            FieldType::Number(NumberFieldType::I64) => i64::PACK_BYTES,
            FieldType::Number(NumberFieldType::I128) => i128::PACK_BYTES,
            FieldType::DateTime => DateTime::PACK_BYTES,
            FieldType::Text => String::PACK_BYTES,
            FieldType::Record { .. } => Ulid::PACK_BYTES,
            FieldType::List { .. } => Vec::<u8>::PACK_BYTES,
            FieldType::Option { .. } => Option::<u8>::PACK_BYTES,
        }
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Number(NumberFieldType::U8) => write!(f, "u8"),
            FieldType::Number(NumberFieldType::U16) => write!(f, "u16"),
            FieldType::Number(NumberFieldType::U32) => write!(f, "u32"),
            FieldType::Number(NumberFieldType::U64) => write!(f, "u64"),
            FieldType::Number(NumberFieldType::U128) => write!(f, "u128"),
            FieldType::Number(NumberFieldType::I8) => write!(f, "i8"),
            FieldType::Number(NumberFieldType::I16) => write!(f, "i16"),
            FieldType::Number(NumberFieldType::I32) => write!(f, "i32"),
            FieldType::Number(NumberFieldType::I64) => write!(f, "i64"),
            FieldType::Number(NumberFieldType::I128) => write!(f, "i128"),
            FieldType::DateTime => write!(f, "datetime"),
            FieldType::Text => write!(f, "Text"),
            FieldType::Record { table_name } => write!(f, "record<{}>", table_name),
            FieldType::List { ty } => write!(f, "list<{}>", ty),
            FieldType::Option { ty } => write!(f, "option<{}>", ty),
        }
    }
}

#[cfg(test)]
mod tests {

    use bytepack::{BytePacker, ByteUnpacker, PackFormat, PackerFormat};

    use crate::table::TableField;

    use super::*;

    #[test]
    fn pack_unpack() {
        let fields = vec![
            FieldType::Number(NumberFieldType::U8),
            FieldType::Text,
            FieldType::Record {
                table_name: "user".into(),
            },
            FieldType::List {
                ty: Box::new(FieldType::List {
                    ty: Box::new(FieldType::Record {
                        table_name: "pet".into(),
                    }),
                }),
            },
        ];

        let format = PackerFormat::new(
            [TableField::new(
                "fields",
                FieldType::List {
                    ty: Box::new(
                        FieldType::Number(NumberFieldType::U8), // Any type is okay here, since only the PACK_SIZE of TableType::List is relevant
                    ),
                },
                false,
            )]
            .into_iter()
            .map(Into::into),
        );

        let mut bytes = BytePacker::new(format.fixed_byte_count());
        let mut packer = bytes.fields(&format, 0);

        packer.pack("fields", &fields);

        let bytes = bytes.finish();

        let unpacker = ByteUnpacker::new(&bytes);
        let unpacker = unpacker.fields(&format, 0);

        let fields_unpack = unpacker.unpack::<Vec<FieldType>>("fields").unwrap();

        assert_eq!(fields_unpack, fields);
    }
}
