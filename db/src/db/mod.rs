mod table_ext;
mod record_ext;
mod index_ext;
mod trigger_ext;
mod query_ext;

use db_core::record::RecordBytes;
pub use index_ext::*;
pub use trigger_ext::*;

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, RwLock},
};

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use ulid::Ulid;

use crate::{Table, db::table_ext::DbTables};

#[derive(Clone)]
pub struct Db {
    inner: Arc<DbInner>,
}

pub struct DbInner {
    db: redb::Database,
    tables: RwLock<DbTables>,
}


type TableWithIdDef<'a> = TableDefinition<'a, u128, &'static [u8]>;

impl Db {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, redb::DatabaseError> {
        let db = Database::create(path)?;

        let inner = DbInner {
            db,
            tables: RwLock::new(Default::default()),
        };

        let this = Self {
            inner: Arc::new(inner),
        };

        this.update_table_map();

        Ok(this)
    }

    pub fn get(&self, table_name: &str, id: Ulid) -> Option<RecordBytes> {
        let tx = self.inner.db.begin_read().ok()?;

        let result;

        {
            let table = tx.open_table(TableWithIdDef::new(table_name)).ok()?;

            let value = table.get(id.0).ok()??;

            let bytes = value.value();

            result = RecordBytes::new(id, bytes.to_owned());
        }

        tx.close().ok()?;

        Some(result)
    }

    pub fn get_all(&self, table_name: &str) -> Option<Vec<RecordBytes>> {
        let tx = self.inner.db.begin_read().ok()?;

        let mut result = Vec::new();

        {
            let table = tx.open_table(TableWithIdDef::new(table_name)).ok()?;

            let values = table.iter().ok()?;

            for v in values {
                let (id, value) = v.ok()?;

                let id = id.value();
                let id = Ulid(id);

                let bytes = value.value();

                result.push(RecordBytes::new(id, bytes.to_owned()));
            }
        }

        tx.close().ok()?;

        Some(result)
    }
}


impl DbTables {

}