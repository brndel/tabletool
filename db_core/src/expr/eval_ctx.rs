use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};

use crate::{defs::table::TableData, record::RecordBytes};

#[derive(Debug, Default)]
pub struct EvalCtx {
    pub records: HashMap<Arc<str>, Arc<RecordBytes>>,
    pub tables: HashMap<Arc<str>, Arc<TableData>>,
    pub now: DateTime<Utc>
}
