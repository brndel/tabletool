use std::mem::transmute;

use crate::asm_code::{asm_code::Literal, asm_pointer::AsmPointer};

pub struct AsmIter {
    pub current_element: AsmPointer,
    pub remaining_elements: u32,
}

#[repr(C)]
struct AsmIterBytes {
    current_element: [u8; AsmPointer::BYTES as usize],
    remaining_elements: [u8; 4],
}

impl AsmIter {
    pub const BYTES: u32 = 12;
    pub const CURRENT_ELEM_PTR_OFFSET: u32 = 0;
    pub const REMAINING_ELEM_OFFSET: u32 = AsmPointer::BYTES;
}

impl From<AsmIter> for AsmIterBytes {
    fn from(value: AsmIter) -> Self {
        AsmIterBytes {
            current_element: value.current_element.into(),
            remaining_elements: value.remaining_elements.to_le_bytes(),
        }
    }
}

impl From<AsmIterBytes> for [u8; AsmIter::BYTES as usize] {
    fn from(value: AsmIterBytes) -> Self {
        unsafe { transmute(value) }
    }
}

impl From<AsmIter> for Literal {
    fn from(value: AsmIter) -> Self {
        let bytes = AsmIterBytes::from(value).into();

        Literal::B12(bytes)
    }
}
