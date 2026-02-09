use std::{collections::BTreeMap, sync::Arc};

use bytepack::{BytePacker, ByteUnpacker, Unpack};
use db_core::{
    defs::{
        index::IndexDef,
        table::{TableData, TableDef, TableFieldData},
        trigger::DbTrigger,
    },
    named::Named,
};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};

use crate::{Db, db::TableWithIdDef};

const TABLE_DEF_TABLE: TableDefinition<'static, &str, &[u8]> = TableDefinition::new("$table");

impl Db {
    pub(super) fn update_table_map(&self) {
        let tx = self.inner.db.begin_read().unwrap();

        let mut table_map = self.inner.tables.write().unwrap();
        *table_map = Default::default();

        let mut table_list = Vec::new();

        {
            let Ok(tables) = tx.open_table(TABLE_DEF_TABLE) else {
                return;
            };

            for table_field in tables.iter().unwrap() {
                let (name, table_fields) = table_field.unwrap();

                let name = name.value();
                let value = {
                    let bytes = table_fields.value();
                    let unpacker = ByteUnpacker::new(bytes);

                    TableDef::unpack(0, &unpacker).unwrap().clone()
                };

                table_list.push(Named {
                    name: name.into(),
                    value,
                });
            }
        }

        table_map.register_tables(table_list);
    }

    pub fn register_table(&self, table: Named<TableDef>) {
        let mut tables = self.inner.tables.write().unwrap();

        let name = &table.name;
        let table_def = &table.value;

        if tables.tables.contains_key(name.as_ref()) {
            panic!("Table already exists");
        }

        let tx = self.inner.db.begin_write().unwrap();

        {
            let mut tables: redb::Table<'_, &str, &[u8]> = tx.open_table(TABLE_DEF_TABLE).unwrap();

            let current_table = tables.get(name.as_ref()).unwrap();

            if let Some(current_table) = current_table {
                let unpacker = ByteUnpacker::new(current_table.value());

                let current_table = TableDef::unpack(0, &unpacker).unwrap();

                if &current_table == table_def {
                    panic!("missmatched table definitions");
                }
            } else {
                drop(current_table);

                let bytes = BytePacker::pack_value(&table_def);

                tables.insert(name.as_ref(), &*bytes).unwrap();
                tx.open_table(TableWithIdDef::new(name.as_ref())).unwrap();
            }
        }

        tx.commit().unwrap();

        tables.register_tables([table]);
    }

    pub fn delete_table(&self, table_name: &str) {
        let mut table_map = self.inner.tables.write().unwrap();

        if !table_map.remove_table(table_name) {
            println!("Unknown table");
            return;
        }

        let tx = self.inner.db.begin_write().unwrap();

        {
            let mut tables = tx.open_table(TABLE_DEF_TABLE).unwrap();

            tables.remove(table_name).unwrap();

            tx.delete_table(TableWithIdDef::new(table_name)).unwrap();
        }

        tx.commit().unwrap();
    }
}

impl Db {
    pub fn table_names(&self) -> Vec<Arc<str>> {
        let tables = self.inner.tables.read().unwrap();

        let names = tables.table_names().cloned().collect();

        names
    }

    pub fn table(&self, name: &str) -> Option<TableData> {
        let tables = self.inner.tables.read().unwrap();

        tables.table(name).cloned()
    }
}

#[derive(Default)]
pub struct DbTables {
    pub tables: BTreeMap<Arc<str>, TableData>,
    pub indices: BTreeMap<Arc<str>, IndexDef>,
    pub triggers: BTreeMap<Arc<str>, Vec<DbTrigger>>,
}

impl DbTables {
    pub fn register_tables(&mut self, tables: impl IntoIterator<Item = Named<TableDef>>) {
        println!("registering tables");
        let mut indices = Vec::new();

        for table in tables {
            let Named { name, value: table } = table;
            let table = TableData::from(table);

            let mut ind = table.indices(&name);
            println!("registering table {} with {} indices", name, ind.len());
            indices.append(&mut ind);

            self.tables.insert(name, table);
        }

        let mut triggers = Vec::new();

        for index in indices {
            let Some(target_field) = self.table_field(&index.table_name, &index.field_name) else {
                println!(
                    "could not get field {} of table {} for index {}",
                    index.field_name, index.table_name, index.index_name
                );
                continue;
            };

            let mut index_triggers = index.triggers(&target_field.ty);

            println!(
                "registering index {} with {} triggers",
                index.index_name,
                index_triggers.len()
            );

            triggers.append(&mut index_triggers);
            self.indices.insert(index.index_name.clone(), index);
        }

        for (table_name, trigger) in triggers {
            println!("registering trigger on table {}: {:?}", table_name, trigger);
            let table_triggers = self.triggers.entry(table_name).or_default();
            table_triggers.push(trigger);
        }
    }

    pub fn remove_table(&mut self, table_name: &str) -> bool {
        let did_remove = self.tables.remove(table_name).is_some();

        if did_remove {
            self.recompute_indices();
            return true;
        } else {
            return false;
        }
    }

    fn recompute_indices(&mut self) {
        self.indices.clear();
        self.triggers.clear();

        let mut indices = Vec::new();

        for (name, table) in &self.tables {
            let mut table_indices = table.indices(name);
            indices.append(&mut table_indices);
        }

        let mut triggers = Vec::new();

        for index in indices {
            let Some(target_field) = self.table_field(&index.table_name, &index.field_name) else {
                println!(
                    "could not get field {} of table {} for index {}",
                    index.field_name, index.table_name, index.index_name
                );
                continue;
            };

            triggers.append(&mut index.triggers(&target_field.ty));
            self.indices.insert(index.index_name.clone(), index);
        }

        for (table_name, trigger) in triggers {
            self.triggers.entry(table_name).or_default().push(trigger);
        }
    }

    pub fn table<'a>(&'a self, name: &str) -> Option<&'a TableData> {
        self.tables.get(name)
    }

    pub fn table_field<'a>(
        &'a self,
        table_name: &str,
        field_name: &str,
    ) -> Option<&'a TableFieldData> {
        self.tables.get(table_name)?.fields.get(field_name)
    }

    pub fn table_names(&self) -> impl Iterator<Item = &Arc<str>> {
        self.tables.keys()
    }
}
