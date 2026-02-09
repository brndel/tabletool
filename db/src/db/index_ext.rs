
use bytepack::PackFormat;
use chrono::{DateTime, Utc};
use db_core::{
    defs::index::IndexOnDelete,
    record::RecordBytes,
    ty::FieldTy,
};
use redb::{MultimapTableDefinition, ReadableDatabase, ReadableMultimapTable, WriteTransaction};
use ulid::Ulid;

use crate::{
    Db, FieldValue,
    db::TableWithIdDef,
    error::DbError,
};

impl Db {
    pub fn index_insert(
        &self,
        tx: &WriteTransaction,
        index_name: &str,
        record: &RecordBytes,
    ) -> Result<(), DbError> {
        let field = {
            let guard = self.inner.tables.read().unwrap();

            let index = guard.indices.get(index_name).unwrap();

            let field = guard
                .table_field(&index.table_name, &index.field_name)
                .unwrap();

            field.clone()
        };

        match field.ty {
            FieldTy::RecordId { table_name } => {
                let mut index = tx.open_multimap_table(
                    MultimapTableDefinition::<'_, u128, u128>::new(index_name),
                )?;

                let field_value = record.unpack::<Ulid>(field.offset).unwrap();

                if !self.record_exists(
                    tx.open_table(TableWithIdDef::new(&table_name))?,
                    field_value,
                )? {
                    return Err(DbError::RecordDoesNotExist {
                        table: table_name.clone(),
                        record: field_value,
                    });
                }

                index.insert(field_value.0, record.id().0)?;
            }
            FieldTy::Timestamp => {
                let field_value = record.unpack::<DateTime<Utc>>(field.offset).unwrap();

                let mut index = tx.open_multimap_table(
                    MultimapTableDefinition::<'_, i64, u128>::new(index_name),
                )?;

                index.insert(field_value.timestamp(), record.id().0)?;
            }
            _ => todo!(),
        }

        Ok(())
    }

    pub fn index_delete_value(
        &self,
        tx: &WriteTransaction,
        index_name: &str,
        record: &RecordBytes,
    ) -> Result<(), DbError> {
        let field = {
            let guard = self.inner.tables.read().unwrap();

            let index = guard.indices.get(index_name).unwrap();

            let field = guard
                .table_field(&index.table_name, &index.field_name)
                .unwrap();

            field.clone()
        };

        match &field.ty {
            FieldTy::RecordId { .. } => {
                let value = record.unpack::<Ulid>(field.offset).unwrap();

                let mut index_table =
                    tx.open_multimap_table(MultimapTableDefinition::<'_, u128, u128>::new(
                        index_name,
                    ))?;

                index_table.remove(value.0, record.id().0)?;

                drop(index_table);
            }
            FieldTy::Timestamp => {
                let field_value = record.unpack::<DateTime<Utc>>(field.offset).unwrap();

                let mut index = tx.open_multimap_table(
                    MultimapTableDefinition::<'_, i64, u128>::new(index_name),
                )?;

                index.remove(field_value.timestamp(), record.id().0)?;
            }
            _ => todo!(),
        }

        Ok(())
    }

    pub fn index_delete_key(
        &self,
        tx: &WriteTransaction,
        index_name: &str,
        id: &Ulid,
    ) -> Result<(), DbError> {
        let (table_field_ty, index) = {
            let guard = self.inner.tables.read().unwrap();

            let index = guard.indices.get(index_name).unwrap();

            let table = guard.tables.get(&index.table_name).unwrap();
            let table_field = table.field(&index.field_name).unwrap();

            (table_field.ty.clone(), index.clone())
        };

        match &table_field_ty {
            FieldTy::RecordId { .. } => {
                let on_delete = &index.on_delete;

                let mut index_table =
                    tx.open_multimap_table(MultimapTableDefinition::<'_, u128, u128>::new(
                        index_name,
                    ))?;

                let mut delete_emit = Vec::new();

                match on_delete {
                    IndexOnDelete::Cascase => {
                        for id in index_table.remove_all(&id.0)? {
                            let mut table =
                                tx.open_table(TableWithIdDef::new(&index.table_name))?;
                            let id = id?.value();
                            let value = table.remove(id)?.unwrap();

                            // TODO  Emit a new delete event for chained data
                            let bytes = value.value().to_owned();

                            delete_emit.push(RecordBytes::new(Ulid(id), bytes));
                        }
                    }
                    IndexOnDelete::SetNone => {
                        unimplemented!()
                    }
                    IndexOnDelete::None => {
                        panic!("Invalid on delete for record")
                    }
                }

                drop(index_table);

                for record in delete_emit {
                    self.emit_delete(&index.table_name, &record, tx)?;
                }
            }
            _ => todo!(),
        }

        Ok(())
    }
}

impl Db {
    pub fn index_query(
        &self,
        index_name: &str,
        min_value: Option<FieldValue>,
        max_value: Option<FieldValue>,
    ) -> Result<Vec<(FieldValue, Ulid)>, DbError> {
        let tx = self.inner.db.begin_read()?;

        let (table_field_ty, index) = {
            let guard = self.inner.tables.read().unwrap();

            let index = guard.indices.get(index_name).unwrap();

            let table = guard.tables.get(&index.table_name).unwrap();
            let table_field = table.field(&index.field_name).unwrap();

            (table_field.ty.clone(), index.clone())
        };

        match table_field_ty {
            FieldTy::RecordId { .. } => {
                let index_table =
                    tx.open_multimap_table(MultimapTableDefinition::<'_, u128, u128>::new(
                        index_name,
                    ))?;

                let mut result = Vec::new();

                for key in index_table.iter()? {
                    let (key, values) = key?;

                    let key_id = Ulid::from(key.value());

                    for value in values {
                        let value = value?.value();

                        let value = Ulid::from(value);

                        result.push((FieldValue::RecordId(key_id), value));
                    }
                }

                Ok(result)
            }
            FieldTy::Timestamp => {
                let index_table = tx.open_multimap_table(
                    MultimapTableDefinition::<'_, i64, u128>::new(index_name),
                )?;

                let min_value = match min_value {
                    Some(FieldValue::DateTime(value)) => Some(value),
                    Some(_) => {
                        return Err(DbError::WrongType {
                            expected: FieldTy::Timestamp,
                        });
                    }
                    None => None,
                };

                let max_value = match max_value {
                    Some(FieldValue::DateTime(value)) => Some(value),
                    Some(_) => {
                        return Err(DbError::WrongType {
                            expected: FieldTy::Timestamp,
                        });
                    }
                    None => None,
                };

                let iter = match (min_value, max_value) {
                    (None, None) => index_table.iter()?,
                    (None, Some(max)) => index_table.range(..max.timestamp())?,
                    (Some(min), None) => index_table.range(min.timestamp()..)?,
                    (Some(min), Some(max)) => {
                        index_table.range(min.timestamp()..max.timestamp())?
                    }
                };

                let mut result = Vec::new();

                for key in iter {
                    let (key, values) = key?;

                    let key = DateTime::from_timestamp(key.value(), 0).unwrap();

                    for value in values {
                        let value = value?.value();

                        let value = Ulid::from(value);

                        result.push((FieldValue::DateTime(key), value));
                    }
                }

                Ok(result)
            }
            _ => todo!(),
        }
    }
}
