use std::{collections::HashMap, sync::Arc};

use crate::defs::table::TableData;


#[derive(Debug, Default)]
pub struct TyCtx {
    pub tables: HashMap<Arc<str>, Arc<TableData>>
}