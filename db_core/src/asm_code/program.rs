use std::{collections::{BTreeMap, HashMap, HashSet}, sync::Arc};

use crate::{asm_code::asm_code::{AccessTableIdx, AsmCode}, ty::Ty};

pub struct Program {
    pub const_memory: Vec<u8>,
    pub code: Vec<AsmCode>,
    pub record_table_indices: HashMap<Arc<str>, u16>,
    pub return_ty: Ty,
    pub access_table_indices: Vec<Arc<str>>,
}
