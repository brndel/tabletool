use std::iter::repeat;

use crate::{PackerFormat, pack::Pack, pack_pointer::PackPointer};

pub struct BytePacker {
    bytes: Vec<u8>,
}

pub struct FieldPacker<'b, 'f> {
    bytes: &'b mut BytePacker,
    format: &'f PackerFormat,
    offset: u32,
}

impl BytePacker {
    pub fn new(fixed_byted_count: u32) -> Self {
        BytePacker { bytes: vec![0; fixed_byted_count as usize] }
    }

    pub fn fields<'b, 'f>(
        &'b mut self,
        format: &'f PackerFormat,
        offset: u32,
    ) -> FieldPacker<'b, 'f> {
        FieldPacker {
            bytes: self,
            format,
            offset,
        }
    }

    pub fn pack_bytes(&mut self, offset: u32, value: &[u8]) {
        pack_fixed_value(&mut self.bytes, offset, value);
    }

    pub fn pack_bytes_indirect(&mut self, offset: u32, value: &[u8]) {
        let ptr = self.push_dynamic(value);
        ptr.pack(offset, self);
    }

    pub fn pack_indirect<T: Pack>(&mut self, offset: u32, value: &T) {
        let ptr = self.reserve_dynamic_bytes(T::PACK_BYTES);
        ptr.pack(offset, self);

        value.pack(ptr.offset, self);
    }

    pub fn push_dynamic(&mut self, value: &[u8]) -> PackPointer {
        push_dynamic_value(&mut self.bytes, value)
    }

    pub fn reserve_dynamic_bytes(&mut self, count: u32) -> PackPointer {
        let offset = self.bytes.len() as u32;

        self.bytes.extend(repeat(0).take(count as usize));

        PackPointer { offset, len: count }
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl BytePacker {

    pub fn pack_value<T: Pack>(value: &T) -> Vec<u8> {
        let mut this = Self::new(T::PACK_BYTES);

        value.pack(0, &mut this);
        
        this.finish()
    }
}

impl<'b, 'f> FieldPacker<'b, 'f> {
    pub fn pack<T: Pack + ?Sized>(&mut self, name: &str, value: &T) {
        let Some(field) = self.format.field(name) else {
            return;
        };

        assert!(T::PACK_BYTES == 0 || field.pointer.len == T::PACK_BYTES);

        value.pack(field.pointer.offset + self.offset, self.bytes);
    }
}

fn pack_fixed_value<'b, 'v>(bytes: &'b mut [u8], offset: u32, value: &'v [u8]) {
    let slice = &mut bytes[(offset as usize)..(offset as usize + value.len())];

    slice.copy_from_slice(value);
}

fn push_dynamic_value(bytes: &mut Vec<u8>, value: &[u8]) -> PackPointer {
    let offset = bytes.len() as u32;
    let len = value.len() as u32;

    bytes.extend_from_slice(value);

    PackPointer { offset, len }
}
