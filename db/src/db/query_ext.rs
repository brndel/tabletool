use std::collections::HashMap;

use crate::{Db, db::TableWithIdDef, error::DbError};

use chrono::Utc;
use db_core::{
    asm_code::{AsmRuntime, compile_expr}, query::{Query, QueryResult, QueryResultGroup, QueryResultRecords}, record::RecordBytes, ty::{FieldTy, Ty}, value::Value
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
        let table_data = table.clone();

        let tx = self.inner.db.begin_read()?;

        let mut result_records = Vec::new();

        {
            let table = tx.open_table(TableWithIdDef::new(&query.table_name))?;

            let filter_program = match &query.filter {
                Some(filter) => {
                    let program = compile_expr(filter, &tables.tables)
                        .map_err(|err| DbError::ExprCompileError)?;

                    if program.return_ty != Ty::Field(FieldTy::Bool) {
                        return Err(DbError::ExprCompileError);
                    }

                    Some(program)
                }
                None => None,
            };

            for entry in table.iter()? {
                let (key, value) = entry?;

                let id = key.value();
                let bytes = value.value();

                let passes_filter = match &filter_program {
                    Some(filter) => {
                        let mut records = vec![None; filter.table_indices.len()];

                        for (table_name, idx) in &filter.table_indices {
                            if table_name == &query.table_name {
                                records[*idx as usize] = Some(bytes)
                            }
                        }

                        let records = records
                            .into_iter()
                            .collect::<Option<Vec<_>>>()
                            .ok_or(DbError::ExprCompileError)?;

                        let mut runtime = AsmRuntime::new(filter, records);

                        runtime.run();

                        runtime.result_bool()
                    }
                    None => true,
                };

                if passes_filter {
                    let record = RecordBytes::new(Ulid::from(id), bytes.to_owned());

                    result_records.push(record);
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

                let program = compile_expr(group_by, &tables.tables)
                    .map_err(|err| DbError::ExprCompileError)?;

                for record in result_records {
                    let mut records = vec![None; program.table_indices.len()];

                    for (table_name, idx) in &program.table_indices {
                        if table_name == &query.table_name {
                            records[*idx as usize] = Some(record.bytes())
                        }
                    }

                    let records = records
                        .into_iter()
                        .collect::<Option<Vec<_>>>()
                        .ok_or(DbError::ExprCompileError)?;

                    let mut runtime = AsmRuntime::new(&program, records);

                    runtime.run();

                    let group = runtime.result().ok_or(DbError::ExprCompileError)?;

                    let entries = groups.entry(group).or_default();

                    entries.push(record);
                }

                return Ok(QueryResult::Grouped {
                    groups: groups
                        .into_iter()
                        .map(|(group_value, records)| QueryResultGroup {
                            group: group_value,
                            result: QueryResult::Records(QueryResultRecords {
                                records,
                                format: table_data.clone(),
                            }),
                        })
                        .collect(),
                });
            }
        }
    }
}
