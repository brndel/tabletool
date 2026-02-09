use bytepack::{Pack, PackPointer, Unpack};



pub enum InlinePointerPack<'a, T> {
    Inline { tag: [u8; 4] },
    Indirect { tag: [u8; 4], value: &'a [u8] },
    Nested { tag: [u8; 4], value: &'a T },
}

impl<'a, T: Pack> Pack for InlinePointerPack<'a, T> {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
         let pointer = match self {
            InlinePointerPack::Inline { tag } => PackPointer::inline(*tag),
            InlinePointerPack::Indirect { tag, value } => {
                let tag_pointer = packer.push_dynamic(tag);
                let value_pointer = packer.push_dynamic(value);

                PackPointer {
                    offset: tag_pointer.offset,
                    len: tag_pointer.len + value_pointer.len,
                }
            }
            InlinePointerPack::Nested { tag, value } => {
                let tag_pointer = packer.push_dynamic(tag);
                let value_pointer = packer.reserve_dynamic_bytes(T::PACK_BYTES);

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

pub enum InlinePointerUnpack<'a> {
    Inline {
        tag: [u8; 4],
    },
    Indirect {
        tag: [u8; 4],
        value: &'a [u8],
        value_offset: u32,
    },
}


impl<'b> Unpack<'b> for InlinePointerUnpack<'b> {
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