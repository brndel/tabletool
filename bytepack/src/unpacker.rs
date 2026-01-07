use crate::{PackField, PackFormat, Unpack, pack_pointer::PackPointer};

pub struct ByteUnpacker<'b> {
    bytes: &'b [u8],
}

impl<'b> ByteUnpacker<'b> {
    pub fn new(bytes: &'b [u8]) -> Self {
        Self { bytes }
    }

    pub fn read_bytes(&self, ptr: PackPointer) -> &'b [u8] {
        read_fixed_value(self.bytes.as_ref(), ptr.offset, ptr.len)
    }

    pub fn read_indirect(&self, offset: u32) -> Option<&'b [u8]> {
        let ptr = PackPointer::unpack(offset, &self)?;
        Some(self.read_bytes(ptr))
    }

    pub fn fields<'f, F>(&self, format: &'f F, offset: u32) -> FieldUnpacker<'b, 'f, F> {
        FieldUnpacker {
            bytes: ByteUnpacker { bytes: self.bytes },
            format,
            offset,
        }
    }
}

fn read_fixed_value<'a>(bytes: &'a [u8], offset: u32, len: u32) -> &'a [u8] {
    &bytes[(offset as usize)..(offset as usize + len as usize)]
}

pub struct FieldUnpacker<'b, 'f, F> {
    bytes: ByteUnpacker<'b>,
    format: &'f F,
    offset: u32,
}

impl<'b, 'f, F: PackFormat> FieldUnpacker<'b, 'f, F> {
    pub fn unpack<T: Unpack<'b>>(&self, name: &str) -> Option<T> {
        let Some(field) = self.format.field(name) else {
            return None;
        };

        T::unpack(field.offset() + self.offset, &self.bytes)
    }
}
