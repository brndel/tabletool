use std::sync::Arc;

use redb::WriteTransaction;

use crate::{Db, RecordBytes, error::DbError};

#[derive(Debug, Clone)]
pub enum DbTrigger {
    OnInsert(TriggerAction),
    OnDelete(TriggerAction),
}

#[derive(Debug, Clone)]
pub enum TriggerAction {
    Println { text: String },
    InsertIntoIndex { index_name: Arc<str> },
    DeleteFromIndex { index_name: Arc<str> },
}

impl Db {
    pub(super) fn emit_insert(
        &self,
        table_name: &str,
        record: &RecordBytes,
        tx: &WriteTransaction,
    ) -> Result<(), DbError> {
        println!("Emit insert for {}:{}", table_name, record.id());
        let Some(table_triggers) = self.get_triggers(table_name) else {
            println!("{} has no triggers", table_name);
            return Ok(());
        };

        println!("{} has {} triggers", table_name, table_triggers.len());

        for trigger in table_triggers {
            if let DbTrigger::OnInsert(action) = trigger {
                self.run_trigger_action(table_name, record, action, tx)?;
            }
        }

        Ok(())
    }

    pub(super) fn emit_delete(
        &self,
        table_name: &str,
        record: &RecordBytes,
        tx: &WriteTransaction,
    ) -> Result<(), DbError> {
        println!("Emit delete for {}:{}", table_name, record.id());
        let Some(table_triggers) = self.get_triggers(table_name) else {
            println!("{} has no triggers", table_name);
            return Ok(());
        };

        println!("{} has {} triggers", table_name, table_triggers.len());

        for trigger in table_triggers {
            if let DbTrigger::OnDelete(action) = trigger {
                self.run_trigger_action(table_name, record, action, tx)?;
            }
        }

        Ok(())
    }

    fn get_triggers(&self, table_name: &str) -> Option<Vec<DbTrigger>> {
        let guard = self.inner.tables.read().unwrap();

        guard.triggers.get(table_name).cloned()
    }

    fn run_trigger_action(
        &self,
        table_name: &str,
        record: &RecordBytes,
        action: TriggerAction,
        tx: &WriteTransaction,
    ) -> Result<(), DbError> {
        println!(
            "Running action {:?} for {}:{}",
            action,
            table_name,
            record.id()
        );
        match action {
            TriggerAction::Println { text } => {
                println!(
                    "trigger println action on {}:{} '{}'",
                    table_name,
                    record.id(),
                    text
                );
                Ok(())
            }
            TriggerAction::InsertIntoIndex { index_name } => {
                self.index_insert(tx, &index_name, record)
            }
            TriggerAction::DeleteFromIndex { index_name } => {
                self.index_delete(tx, &index_name, &record.id())
            }
        }
    }
}
