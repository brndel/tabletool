use std::{collections::HashMap, sync::Arc};

use crate::{defs::table::TableData, record::RecordBytes};

#[derive(Debug, Default)]
pub struct EvalCtx {
    pub records: HashMap<Arc<str>, Arc<RecordBytes>>,
    pub tables: HashMap<Arc<str>, Arc<TableData>>,
}
