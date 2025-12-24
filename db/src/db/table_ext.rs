use std::{collections::BTreeMap, sync::Arc};

use bytepack::{BytePacker, ByteUnpacker, Unpack};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};

use crate::{
    Db, Table,
    db::{DbTrigger, IndexDef, TableWithIdDef},
    table::NamedTable,
};

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

                    Table::unpack(0, &unpacker).unwrap().clone()
                };

                table_list.push(NamedTable::new(name, value));
            }
        }

        table_map.register_tables(table_list);
    }

    pub fn register_table(&self, table: NamedTable) {
        let mut tables = self.inner.tables.write().unwrap();

        if tables.tables.contains_key(table.name()) {
            panic!("Table already exists");
        }

        let tx = self.inner.db.begin_write().unwrap();

        {
            let mut tables: redb::Table<'_, &str, &[u8]> = tx.open_table(TABLE_DEF_TABLE).unwrap();

            let current_table = tables.get(table.name()).unwrap();

            if let Some(current_table) = current_table {
                let unpacker = ByteUnpacker::new(current_table.value());

                let current_table = Table::unpack(0, &unpacker).unwrap();

                if &current_table == &table.table {
                    panic!("missmatched table definitions");
                }
            } else {
                drop(current_table);

                let bytes = BytePacker::pack_value(&table.table);

                tables.insert(table.name(), &*bytes).unwrap();
                tx.open_table(TableWithIdDef::new(table.name())).unwrap();
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

    pub fn table(&self, name: &str) -> Option<Table> {
        let tables = self.inner.tables.read().unwrap();

        let fields = tables.table(name).cloned();

        fields
    }
}

#[derive(Default)]
pub struct DbTables {
    pub tables: BTreeMap<Arc<str>, Table>,
    pub indices: BTreeMap<Arc<str>, IndexDef>,
    pub triggers: BTreeMap<Arc<str>, Vec<DbTrigger>>,
}

impl DbTables {
    pub fn register_tables(&mut self, tables: impl IntoIterator<Item = NamedTable>) {
        println!("registering tables");
        let mut indices = Vec::new();

        for table in tables {
            
            let mut ind = table.indices();
            println!("registering table {} with {} indices", table.name, ind.len());
            indices.append(&mut ind);
            
            self.tables.insert(table.name.clone(), table.table);
        }

        let mut triggers = Vec::new();

        for index in indices {
            let mut trig = index.triggers(&self);
            println!("registering index {} with {} triggers", index.name(), trig.len());
            triggers.append(&mut trig);
            self.indices.insert(index.name().clone(), index);
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
            indices.append(&mut NamedTable::new(name.clone(), table.clone()).indices());
        }

        let mut triggers = Vec::new();

        for index in indices {
            triggers.append(&mut index.triggers(&self));
            self.indices.insert(index.name().clone(), index);
        }

        for (table_name, trigger) in triggers {
            self.triggers.entry(table_name).or_default().push(trigger);
        }
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    pub fn table_names(&self) -> impl Iterator<Item = &Arc<str>> {
        self.tables.keys()
    }
}
