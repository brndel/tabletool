use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{Db, db::TableWithIdDef, error::DbError};

use db_core::{
    defs::table::{TableData, TableDef, TableFieldDef},
    expr::EvalCtx,
    query::Query,
    record::RecordBytes,
};
use redb::{ReadableDatabase, ReadableTable};
use ulid::Ulid;

impl Db {
    pub fn run_query(&self, query: &Query) -> Result<Vec<RecordBytes>, DbError> {
        let tables = self.inner.tables.read().unwrap();
        let Some(table) = tables.tables.get(&query.table_name) else {
            return Err(DbError::TableDoesNotExist {
                table: query.table_name.clone(),
            });
        };

        let table_data = TableData::from(TableDef {
            fields: table
                .fields()
                .iter()
                .map(|field| {
                    let value = (
                        field.name.clone(),
                        TableFieldDef {
                            ty: match &field.ty {
                                crate::FieldType::Number(crate::NumberFieldType::I32) => {
                                    db_core::ty::FieldTy::Int
                                }
                                _ => {
                                    return Err(DbError::WrongType {
                                        expected: crate::FieldType::Number(
                                            crate::NumberFieldType::I32,
                                        ),
                                    });
                                }
                            },
                        },
                    );

                    Ok(value)
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?,
        });

        let tx = self.inner.db.begin_read()?;

        let mut result_records = Vec::new();

        {
            let table = tx.open_table(TableWithIdDef::new(&query.table_name))?;

            let table_name = query.table_name.clone();
            let table_data = Arc::new(table_data);
            let mut tables_map = HashMap::from_iter([(table_name.clone(), table_data.clone())]);

            for entry in table.iter()? {
                let (key, value) = entry?;

                let id = key.value();
                let bytes = value.value();

                let record = RecordBytes::new(Ulid::from(id), bytes.to_owned());
                let record = Arc::new(record);

                if let Some(filter) = &query.filter {
                    let eval_ctx = EvalCtx {
                        records: HashMap::from_iter([(table_name.clone(), record.clone())]),
                        tables: tables_map,
                    };

                    let result = filter.eval(&eval_ctx);

                    drop(eval_ctx.records);

                    if result == Some(db_core::value::Value::Bool(true)) {
                        result_records.push(Arc::into_inner(record).unwrap());
                    }

                    tables_map = eval_ctx.tables;
                }
            }
        }

        Ok(result_records)
    }
}
