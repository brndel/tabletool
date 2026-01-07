use std::sync::Arc;

use crate::PackPointer;

pub trait PackFormat {
    type Field: PackField;

    fn field<'a>(&'a self, name: &str) -> Option<&'a Self::Field>;
    fn fixed_byte_count(&self) -> u32;
}

pub trait PackField {
    fn offset(&self) -> u32;
}

pub struct PackerField {
    pub name: Arc<str>,
    pub size: u32,
}

impl PackerField {
    pub fn new(name: impl Into<Arc<str>>, size: u32) -> Self {
        Self {
            name: name.into(),
            size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldWithPointer {
    pub name: Arc<str>,
    pub pointer: PackPointer,
}

#[derive(Debug, Clone)]
pub struct PackerFormat {
    fields: Vec<FieldWithPointer>,
    fixed_len_byte_count: u32,
}

impl PackerFormat {
    pub fn new(fields: impl IntoIterator<Item = PackerField>) -> Self {
        let mut offset = 0;

        let fields = fields.into_iter().map(|field| {
            let field = FieldWithPointer {
                name: field.name.clone(),
                pointer: PackPointer {
                    offset,
                    len: field.size,
                },
            };

            offset += field.pointer.len;

            field
        });

        Self {
            fields: fields.collect(),
            fixed_len_byte_count: offset,
        }
    }

    pub const fn new_ptrs(fields: Vec<FieldWithPointer>, byte_count: u32) -> Self {
        Self {
            fields,
            fixed_len_byte_count: byte_count,
        }
    }

    pub fn fields<'b>(&'b self) -> &'b [FieldWithPointer] {
        self.fields.as_ref()
    }
}

impl PackFormat for PackerFormat {
    type Field = FieldWithPointer;

    fn field<'a>(&'a self, name: &str) -> Option<&'a Self::Field> {
        self.fields
            .iter()
            .filter(|field| field.name.as_ref() == name)
            .next()
    }

    fn fixed_byte_count(&self) -> u32 {
        self.fixed_len_byte_count
    }
}

impl PackField for FieldWithPointer {
    fn offset(&self) -> u32 {
        self.pointer.offset
    }
}


pub trait HasPackerFormat {
    fn packer_format() -> &'static PackerFormat;
}