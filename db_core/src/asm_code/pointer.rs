use std::mem::transmute;

use crate::asm_code::asm_code::Literal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmPointer {
    pub namespace: Namespace,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Namespace {
    Stack,
    Heap,
    Const,
    Record { idx: u16 },
}

impl From<AsmPointer> for [u8; 8] {
    fn from(value: AsmPointer) -> Self {
        let bytes = AsmPointerBytes::from(value);

        unsafe { transmute(bytes) }
    }
}

impl From<AsmPointer> for Literal {
    fn from(value: AsmPointer) -> Self {
        Self::B64(value.into())
    }
}

impl From<Namespace> for [u8; 4] {
    fn from(value: Namespace) -> Self {
        let bytes = NamespaceBytes::from(value);

        unsafe { transmute(bytes) }
    }
}

impl From<Namespace> for Literal {
    fn from(value: Namespace) -> Self {
        Self::B32(value.into())
    }
}

impl AsmPointer {
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        let bytes: AsmPointerBytes = unsafe { transmute(bytes) };

        bytes.into()
    }

    pub fn add_offset(mut self, offset: u32) -> Self {
        self.offset += offset;

        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmSlicePointer {
    pub pointer: AsmPointer,
    pub len: u32,
}

impl From<AsmSlicePointer> for Literal {
    fn from(value: AsmSlicePointer) -> Self {
        let pointer = AsmSlicePointerBytes {
            pointer: value.pointer.into(),
            len: value.len.to_be_bytes(),
        };

        let result: [u8; 12] = unsafe { transmute(pointer) };

        Literal::B96(result)
    }
}

impl AsmSlicePointer {
    pub const BYTES: u32 = 12;
    pub const POINTER_OFFSET_OFFSET: u32 = 4;
    pub const POINTER_LEN_OFFSET: u32 = 8;

    pub fn from_bytes(bytes: [u8; Self::BYTES as usize]) -> Self {
        let bytes: AsmSlicePointerBytes = unsafe { transmute(bytes) };

        let len = u32::from_be_bytes(bytes.len);

        let pointer = AsmPointer::from(bytes.pointer);

        Self { pointer, len }
    }
}

#[repr(C)]
struct NamespaceBytes {
    namespace_tag: [u8; 2],
    namespace_extra: [u8; 2],
}

impl From<Namespace> for NamespaceBytes {
    fn from(value: Namespace) -> Self {
        let (namespace_tag, idx): (u16, u16) = match value {
            Namespace::Stack => (0, 0),
            Namespace::Heap => (1, 0),
            Namespace::Const => (2, 0),
            Namespace::Record { idx } => (3, idx),
        };

        Self {
            namespace_tag: namespace_tag.to_be_bytes(),
            namespace_extra: idx.to_be_bytes(),
        }
    }
}

impl From<NamespaceBytes> for Namespace {
    fn from(value: NamespaceBytes) -> Self {
        let extra = u16::from_be_bytes(value.namespace_extra);

        let namespace = match u16::from_be_bytes(value.namespace_tag) {
            0 => Namespace::Stack,
            1 => Namespace::Heap,
            2 => Namespace::Const,
            3 => Namespace::Record { idx: extra },
            _ => unreachable!(),
        };

        namespace
    }
}

#[repr(C)]
struct AsmPointerBytes {
    namespace: NamespaceBytes,
    offset: [u8; 4],
}

impl From<AsmPointer> for AsmPointerBytes {
    fn from(value: AsmPointer) -> Self {
        Self {
            namespace: value.namespace.into(),
            offset: value.offset.to_be_bytes(),
        }
    }
}

impl From<AsmPointerBytes> for AsmPointer {
    fn from(value: AsmPointerBytes) -> Self {
        Self {
            namespace: Namespace::from(value.namespace),
            offset: u32::from_be_bytes(value.offset),
        }
    }
}

#[repr(C)]
struct AsmSlicePointerBytes {
    pointer: AsmPointerBytes,
    len: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_pointer_byte_magic() {
        let ptr = AsmSlicePointer {
            pointer: AsmPointer {
                namespace: Namespace::Record { idx: 17265 },
                offset: 876129837,
            },
            len: 1289763,
        };

        let Literal::B96(b) = ptr.into() else {
            panic!()
        };

        let result = AsmSlicePointer::from_bytes(b);

        assert_eq!(ptr, result)
    }
}
