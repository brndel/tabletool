use db_core::record::RecordBytes;
use redb::{ReadableTable, Value};
use ulid::Ulid;

use crate::{Db, db::TableWithIdDef, error::DbError};

impl Db {
    pub fn insert_record(&self, table_name: &str, record: &RecordBytes) -> Result<(), DbError> {
        println!("Inserting {}:{}", table_name, record.id());
        let tx = self.inner.db.begin_write()?;

        {
            let mut table = tx.open_table(TableWithIdDef::new(table_name))?;

            table.insert(record.id().0, record.bytes())?;
        }

        self.emit_insert(table_name, record, &tx)?;

        tx.commit()?;

        Ok(())
    }

    pub fn delete_record(&self, table_name: &str, record_id: Ulid) -> Result<(), DbError> {
        println!("Deleting {}:{}", table_name, record_id);
        let tx = self.inner.db.begin_write()?;

        let bytes = {
            let mut table = tx.open_table(TableWithIdDef::new(table_name))?;

            let Some(value) = table.remove(record_id.0)? else {
                return Err(DbError::RecordDoesNotExist { table: table_name.into(), record: record_id })
            };

            value.value().to_owned()
        };

        self.emit_delete(table_name, &RecordBytes::new(record_id, bytes), &tx)?;

        tx.commit()?;

        Ok(())
    }

    pub(super) fn record_exists<V: Value + 'static>(
        &self,
        table: impl ReadableTable<u128, V>,
        record_id: Ulid,
    ) -> Result<bool, DbError> {
        Ok(table.get(record_id.0)?.is_some())
    }
}
