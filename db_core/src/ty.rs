use std::sync::Arc;

use bytepack::{Pack, PackPointer};

use crate::{defs::table::TableData, named::Named};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FieldTy {
    Int,
    Bool,
}

impl FieldTy {
    pub fn byte_count(&self) -> u32 {
        match self {
            Self::Int => i32::PACK_BYTES,
            Self::Bool => bool::PACK_BYTES,
        }
    }
}

impl Pack for FieldTy {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Ty {
    Field(FieldTy),
    Table(Named<Arc<TableData>>)
}