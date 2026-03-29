use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};

use crate::{Db, db::TableWithIdDef, error::DbError};

use chrono::Utc;
use db_core::{
    asm_code::{
        AccessTableIdx, AsmRuntime, CompilerDiagnostics, CompilerHighlight, CompilerMarker,
        CompletionHint, Program, QueryProvider, compile_expr,
    },
    expr::{Expr, SimpleSpan, Spanned},
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

pub struct CompiledQuery {
    pub diagnostics: CompilerDiagnostics,
    table_name: Arc<str>,
    filter: Option<Program>,
    group_by: Option<Program>,
    group_extra: Option<Program>,
}

impl Db {
    pub fn compile_query(&self, query: &Query) -> Result<CompiledQuery, CompilerDiagnostics> {
        let mut diagnostics = CompilerDiagnostics::new();

        let tables = self.inner.tables.read().unwrap();

        let Some((table_name, table)) = tables.tables.get_key_value(&query.table_name.value) else {
            diagnostics.add_marker(Spanned::new(
                query.table_name.span,
                CompilerMarker::Custom {
                    message: format!("table {} does not exist", query.table_name.value),
                    kind: db_core::asm_code::MarkerKind::Error,
                },
            ));

            diagnostics.add_completion(Spanned::new(
                query.table_name.span,
                CompletionHint {
                    options: tables
                        .tables
                        .keys()
                        .map(|table_name| table_name.to_string())
                        .collect(),
                },
            ));

            return Err(diagnostics);
        };

        let table_data = table.clone();

        {
            let mut table_format = String::from("{\n  ");

            for (idx, field) in table_data.fields().enumerate() {
                if idx != 0 {
                    table_format += ",\n  "
                }
                write!(&mut table_format, "{}: {:?}", field.name, field.value.ty).unwrap();
            }

            table_format += "\n}";

            diagnostics.add_highlight(Spanned::new(
                query.table_name.span,
                CompilerHighlight {
                    message: format!("table {table_format}"),
                },
            ));
        }

        let filter = query.filter.as_ref().map(|expr| {
            Some(Spanned::new(
                expr.span,
                compile_expr(
                    &expr.value,
                    &tables.tables,
                    &HashSet::new(),
                    &mut diagnostics,
                )?,
            ))
        });

        let group_by = query.group_by.as_ref().map(|expr| {
            compile_expr(
                &expr.value,
                &tables.tables,
                &HashSet::new(),
                &mut diagnostics,
            )
        });

        let group_extra = query.group_extra.as_ref().map(|expr| {
            compile_expr(
                &expr.value,
                &tables.tables,
                &HashSet::from_iter([table_name.clone()]),
                &mut diagnostics,
            )
        });

        if let Some(Some(program)) = filter.as_ref() {
            if program.value.return_ty != Ty::Field(FieldTy::Bool) {
                diagnostics.add_marker(Spanned::new(
                    program.span,
                    CompilerMarker::Custom {
                        message: "where expr does not eval to bool".to_owned(),
                        kind: db_core::asm_code::MarkerKind::Error,
                    },
                ));

                return Err(diagnostics);
            }
        }

        fn map_some_some<T>(x: Option<Option<T>>) -> Result<Option<T>, ()> {
            match x {
                Some(None) => Err(()),
                Some(Some(x)) => Ok(Some(x)),
                None => Ok(None),
            }
        }

        let (filter, group_by, group_extra) = match (
            map_some_some(filter),
            map_some_some(group_by),
            map_some_some(group_extra),
        ) {
            (Ok(filter), Ok(group_by), Ok(group_extra)) => {
                (filter.map(|filter| filter.value), group_by, group_extra)
            }
            _ => return Err(diagnostics),
        };

        Ok(CompiledQuery {
            diagnostics,
            table_name: table_name.clone(),
            filter,
            group_by,
            group_extra,
        })
    }

    pub fn run_query(&self, query: &CompiledQuery) -> Result<QueryResult, DbError> {
        let mut diagnostics = CompilerDiagnostics::new();
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
                    let record_query = RecordQuery::new(&filter, &tx);

                    Some((filter, record_query))
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
                            .ok_or(DbError::ExprError(
                                "not all record_table_indices records are filled",
                            ))?;

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
            Some(program) => {
                #[derive(Default)]
                struct GroupData {
                    extra: Option<Value>,
                    records: Vec<RecordBytes>,
                }

                let mut groups = HashMap::<Value, GroupData>::new();

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
                        .ok_or(DbError::ExprError(
                            "not all record_table_indices records are filled",
                        ))?;

                    let mut runtime = AsmRuntime::new(&program, records, &mut record_query, []);

                    runtime.run();

                    let group = runtime.result().map_err(DbError::ExprPanic)?;

                    let entries = groups.entry(group).or_default();

                    entries.records.push(record);
                }

                if let Some(program) = &query.group_extra {
                    assert!(program.record_table_indices.is_empty());
                    assert!(program.access_table_indices.len() <= 1);
                    assert!(
                        program.access_table_indices.len() == 0
                            || &program.access_table_indices[0] == &query.table_name
                    );

                    let mut record_query = RecordQuery::new(&program, &tx);

                    for (_, group) in &mut groups {
                        // let mut records = vec![None; program.record_table_indices.len()];

                        // for (table_name, idx) in &program.record_table_indices {
                        //     if table_name == &query.table_name {
                        //         records[*idx as usize] = Some(record.bytes())
                        //     }
                        // }

                        let records = group
                            .records
                            .iter()
                            .map(|record| Cow::Borrowed(record.bytes()))
                            .collect::<Vec<_>>();

                        let mut runtime = AsmRuntime::new(
                            &program,
                            records,
                            &mut record_query,
                            [(AccessTableIdx(0), 0, group.records.len() as u32)],
                        );

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
        let mut diagnostics = CompilerDiagnostics::new();
        let now = Utc::now();

        let tables = self.inner.tables.read().unwrap();

        let tx = self.inner.db.begin_read()?;

        let program =
            compile_expr(expr, &tables.tables, &HashSet::new(), &mut diagnostics).unwrap();

        if !program.record_table_indices.is_empty() {
            return Err(DbError::ExprError(
                "expression is not allowed to contain table accesses",
            ));
        }

        let mut record_query = RecordQuery::new(&program, &tx);

        let mut runtime = AsmRuntime::new(&program, Vec::new(), &mut record_query, []);

        runtime.run();

        runtime.result().map_err(DbError::ExprPanic)
    }
}
