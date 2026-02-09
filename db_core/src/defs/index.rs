use std::sync::Arc;

use crate::{defs::trigger::{DbTrigger, TriggerAction}, ty::FieldTy};

#[derive(Debug, Clone)]
pub struct IndexDef {
    pub index_name: Arc<str>,
    pub table_name: Arc<str>,
    pub field_name: Arc<str>,
    pub on_delete: IndexOnDelete,
}

#[derive(Debug, Clone, Copy)]
pub enum IndexOnDelete {
    None,
    Cascase,
    SetNone,
}

impl IndexDef {
    pub fn triggers(&self, target_field_ty: &FieldTy) -> Vec<(Arc<str>, DbTrigger)> {
        let mut result = vec![
            (
                self.table_name.clone(),
                DbTrigger::OnInsert(TriggerAction::InsertIntoIndex {
                    index_name: self.index_name.clone(),
                }),
            ),
            (
                self.table_name.clone(),
                DbTrigger::OnDelete(TriggerAction::DeleteValueFromIndex {
                    index_name: self.index_name.clone(),
                }),
            ),
        ];

        // let source_table = get_table(&self.table_name)?;
        // let field_ty = &source_table.fields.get(&self.field_name)?.ty;

        if let FieldTy::RecordId { table_name } = &target_field_ty {
            result.push((
                table_name.clone(),
                DbTrigger::OnDelete(TriggerAction::DeleteKeyFromIndex {
                    index_name: self.index_name.clone(),
                }),
            ));
        }

        result
    }
}
