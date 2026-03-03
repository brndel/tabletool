use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{Db, db::TableWithIdDef, error::DbError};

use chrono::Utc;
use db_core::{
    asm_code::{AccessTableIdx, AsmRuntime, Program, QueryProvider, compile_expr},
    expr::Expr,
    query::{Query, QueryResult, QueryResultGroup, QueryResultRecords},
    record::RecordBytes,
    ty::{FieldTy, Ty},
    value::Value,
};
use redb::{ReadOnlyTable, ReadTransaction, ReadableDatabase, ReadableTable};
use ulid::Ulid;

struct RecordQuery {
    tables: Vec<RecordQueryTable>,
}

struct RecordQueryTable {
    table: ReadOnlyTable<u128, &'static [u8]>,
    records: HashMap<Ulid, Vec<u8>>,
}

impl RecordQuery {
    pub fn new(program: &Program, tx: &ReadTransaction) -> Self {
        let tables = program
            .access_table_indices
            .iter()
            .map(|index| {
                let table = tx.open_table(TableWithIdDef::new(&index)).unwrap();

                RecordQueryTable {
                    table,
                    records: Default::default(),
                }
            })
            .collect();
        Self { tables }
    }
}

impl QueryProvider for RecordQuery {
    fn get_record(&mut self, table_idx: AccessTableIdx, id: Ulid) -> Option<Vec<u8>> {
        let table = self.tables.get_mut(table_idx.0 as usize)?;

        if let Some(value) = table.records.get(&id) {
            return Some(value.clone());
        } else {
            let record = table.table.get(id.0).ok()??;

            let value = record.value().to_owned();
            table.records.insert(id, value.clone());

            Some(value)
        }
    }
}

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

            let mut filter_program = match &query.filter {
                Some(filter) => {
                    let program = compile_expr(filter, &tables.tables, &HashSet::new())
                        .map_err(DbError::ExprCompileError)?;

                    if program.return_ty != Ty::Field(FieldTy::Bool) {
                        return Err(DbError::ExprError("'where' expression does not return bool"));
                    }

                    let record_query = RecordQuery::new(&program, &tx);

                    Some((program, record_query))
                }
                None => None,
            };

            for entry in table.iter()? {
                let (key, value) = entry?;

                let id = Ulid(key.value());
                let bytes = value.value();

                let passes_filter = match &mut filter_program {
                    Some((filter, record_query)) => {
                        let mut records = vec![None; filter.record_table_indices.len()];

                        for (table_name, idx) in &filter.record_table_indices {
                            if table_name == &query.table_name {
                                records[*idx as usize] = Some(bytes)
                            }
                        }

                        let records = records
                            .into_iter()
                            .map(|record| record.map(Cow::Borrowed))
                            .collect::<Option<Vec<_>>>()
                            .ok_or(DbError::ExprError("not all record_table_indices records are filled"))?;

                        let mut runtime = AsmRuntime::new(filter, records, record_query, []);

                        runtime.run();

                        runtime.result_bool()
                    }
                    None => true,
                };

                if passes_filter {
                    let record = RecordBytes::new(id, bytes.to_owned());

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
                #[derive(Default)]
                struct GroupData {
                    extra: Option<Value>,
                    records: Vec<RecordBytes>,
                }

                let mut groups = HashMap::<Value, GroupData>::new();

                let program = compile_expr(group_by, &tables.tables, &HashSet::new())
                    .map_err(DbError::ExprCompileError)?;

                let mut record_query = RecordQuery::new(&program, &tx);

                for record in result_records {
                    let mut records = vec![None; program.record_table_indices.len()];

                    for (table_name, idx) in &program.record_table_indices {
                        if table_name == &query.table_name {
                            records[*idx as usize] = Some(record.bytes())
                        }
                    }

                    let records = records
                        .into_iter()
                        .map(|record| record.map(Cow::Borrowed))
                        .collect::<Option<Vec<_>>>()
                        .ok_or(DbError::ExprError("not all record_table_indices records are filled"))?;

                    let mut runtime = AsmRuntime::new(&program, records, &mut record_query, []);

                    runtime.run();

                    let group = runtime.result().map_err(DbError::ExprPanic)?;

                    let entries = groups.entry(group).or_default();

                    entries.records.push(record);
                }

                if let Some(group_extra) = &query.group_extra {
                    let program = compile_expr(
                        group_extra,
                        &tables.tables,
                        &HashSet::from_iter([query.table_name.clone()]),
                    )
                    .map_err(DbError::ExprCompileError)?;

                    assert!(program.record_table_indices.is_empty());
                    assert!(program.access_table_indices.len() <= 1);
                    assert!(program.access_table_indices.len() == 0 || &program.access_table_indices[0] == &query.table_name);

                    let mut record_query = RecordQuery::new(&program, &tx);

                    for (_, group) in &mut groups {
                        // let mut records = vec![None; program.record_table_indices.len()];

                        // for (table_name, idx) in &program.record_table_indices {
                        //     if table_name == &query.table_name {
                        //         records[*idx as usize] = Some(record.bytes())
                        //     }
                        // }

                        let records = group.records
                            .iter()
                            .map(|record| Cow::Borrowed(record.bytes()))
                            .collect::<Vec<_>>();

                        let mut runtime =
                            AsmRuntime::new(&program, records, &mut record_query, [(AccessTableIdx(0), 0, group.records.len() as u32)]);

                        runtime.run();

                        group.extra = Some(runtime.result().unwrap())
                    }
                }

                return Ok(QueryResult::Grouped {
                    groups: groups
                        .into_iter()
                        .map(|(group_value, group)| QueryResultGroup {
                            group: group_value,
                            extra: group.extra,
                            result: QueryResult::Records(QueryResultRecords {
                                records: group.records,
                                format: table_data.clone(),
                            }),
                        })
                        .collect(),
                });
            }
        }
    }

    pub fn run_expr(&self, expr: &Expr) -> Result<Value, DbError> {
        let now = Utc::now();

        let tables = self.inner.tables.read().unwrap();

        let tx = self.inner.db.begin_read()?;

        let program = compile_expr(expr, &tables.tables, &HashSet::new())
            .map_err(DbError::ExprCompileError)?;

        if !program.record_table_indices.is_empty() {
            return Err(DbError::ExprError("expression is not allowed to contain table accesses"));
        }

        let mut record_query = RecordQuery::new(&program, &tx);

        let mut runtime = AsmRuntime::new(&program, Vec::new(), &mut record_query, []);

        runtime.run();

        runtime.result().map_err(DbError::ExprPanic)
    }
}
