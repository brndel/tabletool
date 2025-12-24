use std::sync::Arc;

use crate::PackPointer;

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
    pub fn new(fields: impl Iterator<Item = PackerField>) -> Self {
        let mut ptr_fields = Vec::new();
        let mut total_offset = 0;

        for field in fields {
            let offset = total_offset;
            let byte_count = field.size;
            total_offset += byte_count;

            ptr_fields.push(FieldWithPointer {
                name: field.name.clone(),
                pointer: PackPointer {
                    offset,
                    len: byte_count,
                },
            });
        }

        Self {
            fields: ptr_fields.into(),
            fixed_len_byte_count: total_offset,
        }
    }

    pub const fn new_ptrs(fields: Vec<FieldWithPointer>, byte_count: u32) -> Self {
        Self {
            fields,
            fixed_len_byte_count: byte_count,
        }
    }

    pub const fn fixed_byte_count(&self) -> u32 {
        self.fixed_len_byte_count
    }

    pub fn field<'b>(&'b self, name: &str) -> Option<&'b FieldWithPointer> {
        self.fields.iter().filter(|field| field.name.as_ref() == name).next()
    }

    pub fn fields<'b>(&'b self) -> &'b [FieldWithPointer] {
        self.fields.as_ref()
    }
}
