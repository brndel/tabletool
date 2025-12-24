use std::sync::Arc;

use bytepack::{ByteUnpacker, Unpack};
use redb::{MultimapTableDefinition, WriteTransaction};
use ulid::Ulid;

use crate::{
    Db, FieldType, RecordBytes,
    db::{
        TableWithIdDef, table_ext::DbTables, trigger_ext::{DbTrigger, TriggerAction}
    },
    error::DbError,
};

#[derive(Debug, Clone)]
pub struct IndexDef {
    name: Arc<str>,
    table: Arc<str>,
    field: Arc<str>,
    on_delete: IndexOnDelete,
}

#[derive(Debug, Clone, Copy)]
pub enum IndexOnDelete {
    Cascase,
    SetNone,
}

impl Db {
    pub fn index_insert(
        &self,
        tx: &WriteTransaction,
        index_name: &str,
        record: &RecordBytes,
    ) -> Result<(), DbError> {
        let (table_field_ptr, table_field_ty) = {
            let guard = self.inner.tables.read().unwrap();

            let index = guard.indices.get(index_name).unwrap();

            let table = guard.tables.get(&index.table).unwrap();
            let format = table.packer_format();
            let table_field = table.field(&index.field).unwrap();
            let packer_field = format.field(&index.field).unwrap();

            (packer_field.pointer, table_field.ty.clone())
        };

        match table_field_ty {
            FieldType::Record { table_name } => {
                let mut index = tx.open_multimap_table(
                    MultimapTableDefinition::<'_, u128, u128>::new(index_name),
                )?;

                let unpacker = ByteUnpacker::new(record.bytes());
                let field_value = Ulid::unpack(table_field_ptr.offset, &unpacker).unwrap();

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
            _ => todo!(),
        }

        Ok(())
    }

    pub fn index_delete(
        &self,
        tx: &WriteTransaction,
        index_name: &str,
        id: &Ulid,
    ) -> Result<(), DbError> {
        let (table_field_ty, index) = {
            let guard = self.inner.tables.read().unwrap();

            let index = guard.indices.get(index_name).unwrap();

            let table = guard.tables.get(&index.table).unwrap();
            let table_field = table.field(&index.field).unwrap();

            (table_field.ty.clone(), index.clone())
        };

        match &table_field_ty {
            FieldType::Record { table_name } => {
                let on_delete = &index.on_delete;

                let mut index_table =
                    tx.open_multimap_table(MultimapTableDefinition::<'_, u128, u128>::new(
                        index_name,
                    ))?;

                let mut delete_emit = Vec::new();

                match on_delete {
                    IndexOnDelete::Cascase => {
                        
                        for id in index_table.remove_all(&id.0)? {
                            let mut table = tx.open_table(TableWithIdDef::new(&index.table))?;
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
                }

                drop(index_table);

                for record in delete_emit {
                    self.emit_delete(&index.table, &record, tx)?;
                }
            }
            _ => todo!(),
        }

        Ok(())
    }
}

impl IndexDef {
    pub fn new(
        index_name: Arc<str>,
        table: Arc<str>,
        field: Arc<str>,
        on_delete: IndexOnDelete,
    ) -> Self {
        Self {
            name: index_name,
            table,
            field,
            on_delete,
        }
    }

    pub fn name(&self) -> &Arc<str> {
        &self.name
    }

    pub fn triggers(&self, db: &DbTables) -> Vec<(Arc<str>, DbTrigger)> {
        let mut result = Vec::with_capacity(2);

        result.push((
            self.table.clone(),
            DbTrigger::OnInsert(TriggerAction::InsertIntoIndex {
                index_name: self.name.clone(),
            }),
        ));

        let source_table = db.table(&self.table).unwrap();
        let field_ty = &source_table.field(&self.field).unwrap().ty;

        if let FieldType::Record { table_name } = &field_ty {
            result.push((
                table_name.clone(),
                DbTrigger::OnDelete(TriggerAction::DeleteFromIndex {
                    index_name: self.name.clone(),
                }),
            ));
        }

        result
    }
}
