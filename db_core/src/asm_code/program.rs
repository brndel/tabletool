use std::{collections::BTreeMap, sync::Arc};

use crate::{asm_code::asm_code::AsmCode, ty::Ty};

pub struct Program {
    pub const_memory: Vec<u8>,
    pub code: Vec<AsmCode>,
    pub table_indices: BTreeMap<Arc<str>, u16>,
    pub return_ty: Ty,
}
