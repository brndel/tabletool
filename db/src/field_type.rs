use std::{fmt::Display, sync::Arc};

use chrono::DateTime;
use db_core::inline_pointer::{InlinePointerPack, InlinePointerUnpack};
use ulid::Ulid;

use bytepack::{Pack, Unpack};

type InlinePointerFieldType<'a> = InlinePointerPack<'a, FieldType>;

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
    const PACK_BYTES: u32 = InlinePointerFieldType::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        self.pointer_value().pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for FieldType {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let pointer_value = InlinePointerUnpack::unpack(offset, unpacker)?;
        match pointer_value {
            InlinePointerUnpack::Inline { tag } => match &tag {
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
            InlinePointerUnpack::Indirect {
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

impl FieldType {
    fn pointer_value(&self) -> InlinePointerFieldType<'_> {
        match self {
            FieldType::Record { table_name } => InlinePointerFieldType::Indirect {
                tag: *b"rcrd",
                value: table_name.as_bytes(),
            },
            FieldType::List { ty } => InlinePointerFieldType::Nested {
                tag: *b"list",
                value: ty,
            },
            FieldType::Option { ty } => InlinePointerFieldType::Nested {
                tag: *b"opti",
                value: ty,
            },
            FieldType::Text => InlinePointerFieldType::Inline { tag: *b"text" },
            FieldType::DateTime => InlinePointerFieldType::Inline { tag: *b"dati" },
            FieldType::Number(NumberFieldType::U8) => {
                InlinePointerFieldType::Inline { tag: *b"u8  " }
            }
            FieldType::Number(NumberFieldType::U16) => {
                InlinePointerFieldType::Inline { tag: *b"u16 " }
            }
            FieldType::Number(NumberFieldType::U32) => {
                InlinePointerFieldType::Inline { tag: *b"u32 " }
            }
            FieldType::Number(NumberFieldType::U64) => {
                InlinePointerFieldType::Inline { tag: *b"u64 " }
            }
            FieldType::Number(NumberFieldType::U128) => {
                InlinePointerFieldType::Inline { tag: *b"u128" }
            }
            FieldType::Number(NumberFieldType::I8) => {
                InlinePointerFieldType::Inline { tag: *b"i8  " }
            }
            FieldType::Number(NumberFieldType::I16) => {
                InlinePointerFieldType::Inline { tag: *b"i16 " }
            }
            FieldType::Number(NumberFieldType::I32) => {
                InlinePointerFieldType::Inline { tag: *b"i32 " }
            }
            FieldType::Number(NumberFieldType::I64) => {
                InlinePointerFieldType::Inline { tag: *b"i64 " }
            }
            FieldType::Number(NumberFieldType::I128) => {
                InlinePointerFieldType::Inline { tag: *b"i128" }
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
    use db_core::defs::table::TableFieldDef;


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
            [TableFieldDef::new(
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
