use std::{collections::HashMap, sync::Arc};

use crate::{Db, db::TableWithIdDef, error::DbError};

use chrono::Utc;
use db_core::{expr::EvalCtx, query::Query, record::RecordBytes};
use redb::{ReadableDatabase, ReadableTable};
use ulid::Ulid;

impl Db {
    pub fn run_query(&self, query: &Query) -> Result<Vec<RecordBytes>, DbError> {
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

        {
            let table = tx.open_table(TableWithIdDef::new(&query.table_name))?;

            let table_name = query.table_name.clone();
            let mut tables_map = HashMap::from_iter([(table_name.clone(), table_data.clone())]);

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

                    if result == Some(db_core::value::Value::Bool(true)) {
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

        Ok(result_records)
    }
}
