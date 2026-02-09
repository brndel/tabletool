use std::sync::Arc;


#[derive(Debug, Clone)]
pub enum DbTrigger {
    OnInsert(TriggerAction),
    OnDelete(TriggerAction),
}

#[derive(Debug, Clone)]
pub enum TriggerAction {
    Println {
        text: String,
    },
    InsertIntoIndex {
        index_name: Arc<str>,
    },
    DeleteValueFromIndex {
        index_name: Arc<str>,
    },
    DeleteKeyFromIndex {
        index_name: Arc<str>,
    },
}