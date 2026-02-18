use std::{collections::HashMap, sync::Arc};

use crate::{Db, db::TableWithIdDef, error::DbError};

use chrono::Utc;
use db_core::{
    expr::EvalCtx,
    query::{Query, QueryResult, QueryResultGroup, QueryResultRecords},
    record::RecordBytes,
    value::{FieldValue, Value},
};
use redb::{ReadableDatabase, ReadableTable};
use ulid::Ulid;

impl Db {
    pub fn run_query(&self, query: &Query) -> Result<QueryResult, DbError> {
        let now = Utc::now();

        let tables = self.inner.tables.read().unwrap();

        let Some(table) = tables.tables.get(&query.table_name) else {
            return Err(DbError::TableDoesNotExist {
                table: query.table_name.clone(),
            });
        };
        let table_data = Arc::new(table.clone());

        let tx = self.inner.db.begin_read()?;

        let mut result_records = Vec::new();

        let table_name = query.table_name.clone();
        let mut tables_map = HashMap::from_iter([(table_name.clone(), table_data.clone())]);

        {
            let table = tx.open_table(TableWithIdDef::new(&query.table_name))?;

            for entry in table.iter()? {
                let (key, value) = entry?;

                let id = key.value();
                let bytes = value.value();

                let record = RecordBytes::new(Ulid::from(id), bytes.to_owned());
                let record = Arc::new(record);

                let passes_filter = if let Some(filter) = &query.filter {
                    let eval_ctx = EvalCtx {
                        records: HashMap::from_iter([(table_name.clone(), record.clone())]),
                        tables: tables_map,
                        now,
                    };

                    let result = filter.eval(&eval_ctx);

                    drop(eval_ctx.records);
                    tables_map = eval_ctx.tables;

                    dbg!(&result);

                    if let Ok(result) = result
                        && result == Value::Field(FieldValue::Bool(true))
                    {
                        true
                    } else {
                        false
                    }
                } else {
                    true
                };

                if passes_filter {
                    result_records.push(Arc::into_inner(record).unwrap());
                }
            }
        }

        match &query.group_by {
            None => {
                return Ok(QueryResult::Records(QueryResultRecords {
                    records: result_records,
                    format: table_data,
                }));
            }
            Some(group_by) => {
                let mut groups = HashMap::<Value, Vec<RecordBytes>>::new();

                for record in result_records {
                    let record = Arc::new(record);

                    let eval_ctx = EvalCtx {
                        records: HashMap::from_iter([(table_name.clone(), record.clone())]),
                        tables: tables_map,
                        now,
                    };

                    let group_value = group_by.eval(&eval_ctx);

                    drop(eval_ctx.records);
                    tables_map = eval_ctx.tables;

                    if let Ok(group) = group_value {
                        let record = Arc::into_inner(record).unwrap();

                        let entries = groups.entry(group).or_default();

                        entries.push(record);
                    }
                }

                return Ok(QueryResult::Grouped {
                    groups: groups
                        .into_iter()
                        .map(|(group_value, records)| QueryResultGroup {
                            group: group_value,
                            result: QueryResult::Records(QueryResultRecords { records, format: table_data.clone() }),
                        })
                        .collect(),
                });
            }
        }
    }
}
