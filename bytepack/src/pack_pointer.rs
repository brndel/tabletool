use std::mem::transmute;

use crate::{BytePacker, Pack, Unpack};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackPointer {
    pub offset: u32,
    pub len: u32,
}

impl PackPointer {
    pub const NULL: Self = PackPointer { offset: 0, len: 0 };

    pub const fn inline(tag: [u8; 4]) -> Self {
        Self {
            offset: 0,
            len: u32::from_be_bytes(tag),
        }
    }

    pub const fn inline_tag(&self) -> Option<[u8; 4]> {
        if self.offset == 0 {
            Some(self.len.to_be_bytes())
        } else {
            None
        }
    }
}

impl Pack for PackPointer {
    const PACK_BYTES: u32 = (32 + 32) / 8;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        let bytes =
            unsafe { transmute::<_, [u8; 8]>([self.offset.to_be_bytes(), self.len.to_be_bytes()]) };

        packer.pack_bytes(offset, bytes.as_ref());
    }
}

impl<'b> Unpack<'b> for PackPointer {
    fn unpack(offset: u32, unpacker: &super::ByteUnpacker<'b>) -> Option<Self> {
        let bytes = unpacker.read_bytes(PackPointer {
            offset,
            len: Self::PACK_BYTES,
        });

        let ptr = <[u8; 8]>::try_from(bytes).unwrap();
        let [offset_bytes, len_bytes] = unsafe { transmute::<_, [[u8; 4]; 2]>(ptr) };

        let offset = u32::from_be_bytes(offset_bytes);
        let len = u32::from_be_bytes(len_bytes);

        Some(Self { offset, len })
    }
}
