use std::sync::Arc;

use dioxus_stores::Store;

use crate::{defs::table::TableData, record::RecordBytes, value::Value};

#[derive(Debug, Clone, Store)]
pub enum QueryResult {
    Records(QueryResultRecords),
    Grouped { groups: Vec<QueryResultGroup> },
}

#[derive(Debug, Clone, Store)]
pub struct QueryResultRecords {
    pub records: Vec<RecordBytes>,
    pub format: Arc<TableData>,
}

#[derive(Debug, Clone, Store)]
pub struct QueryResultGroup {
    pub group: Value,
    pub result: QueryResult,
}
