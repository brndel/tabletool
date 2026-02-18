use chrono::{DateTime, Local, Utc};
use db::{Db, Ulid};
use db_core::{
    defs::table::TableFieldData, named::Named, query::QueryResultRecords, record::RecordBytes, value::{FieldValue, Value}
};
use dioxus::prelude::*;

use crate::{
    components::{
        alert_dialog::{
            AlertDialogAction, AlertDialogActions, AlertDialogCancel, AlertDialogContent,
            AlertDialogDescription, AlertDialogRoot, AlertDialogTitle,
        },
        button::{Button, ButtonVariant},
    },
    id_card::{IdCard, id_text},
};

#[component]
pub fn DataTable(
    records: ReadSignal<QueryResultRecords>,
    delete: Option<Callback<Ulid, ()>>,
    table_name: String,
) -> Element {
    let mut is_delete_dialog_open = use_signal(|| None);
    let mut selected_id = use_signal(|| None);

    let display_field_idx = records.read().format.main_display_field_idx();

    let db = use_context::<Db>();

    rsx!(
        table {
            thead {
                tr {
                    th {"Id"}
                    for (field_idx, Named { name: field_name, value: field }) in records.read().format.fields().enumerate() {
                        th {
                            Button {
                                onclick: {
                                    let has_index = field.has_index;
                                    let field_name = field_name.clone();
                                    let db = db.clone();
                                    let table_name = table_name.clone();
                                    move |_| if has_index {
                                        let query_result = db.index_query(&format!("#{}:{}", table_name, field_name), Some(FieldValue::Timestamp(Utc::now())), None);

                                        match query_result {
                                            Ok(result) => {
                                                for (index_value, record_key) in result {
                                                    println!("{:?}:{}", index_value, record_key);
                                                }
                                            }
                                            Err(err) => {
                                                println!("ERROR: {err}");
                                            }
                                        }
                                    }
                                },
                                variant: ButtonVariant::Ghost,
                                if field.has_index {
                                    "#"
                                }
                                "{field_name}"
                                if Some(field_idx) == display_field_idx {
                                    "*"
                                }
                            }
                        }
                    }
                    if delete.is_some() {
                        th {"…"}
                    }
                }
            }
            tbody {
                for record in records.read().records.iter() {
                    tr { key: "{record.id()}",
                        td {
                            IdCard {
                                id: record.id()
                            }
                        }
                        for field in records.read().format.fields() {
                            td {
                                "{extract_value(record, &field.value, &db)}"
                            }
                        }
                        if delete.is_some() {
                            td {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: {
                                        let id = record.id();
                                        move |_| {
                                            selected_id.set(Some(id));
                                            is_delete_dialog_open.set(Some(true));
                                        }
                                    },
                                    "Delete"
                                }
                            }
                        }
                    }
                }
            }
        }

        AlertDialogRoot {
            open: is_delete_dialog_open,
            on_open_change: move |v| is_delete_dialog_open.set(Some(v)),
            AlertDialogContent {
                AlertDialogTitle { "Delete item" }
                AlertDialogDescription { "Are you sure you want to delete this item? This action cannot be undone." }
                AlertDialogActions {
                    AlertDialogCancel { "Cancel" }
                    AlertDialogAction { on_click:
                        move |_| {
                            if let Some(delete) = delete && let Some(id) = *selected_id.read() {
                                delete(id);
                            }
                            selected_id.set(None);
                        }
                    ,
                    "Delete"
                    }
                }
            }
        }
    )
}

pub fn extract_value(record: &RecordBytes, field: &TableFieldData, db: &Db) -> String {
    match record.get_field(field) {
        Some(value) => field_value_to_string(value, db),
        None => "???".to_string(),
    }
}

pub fn value_to_string(value: Value, db: &Db) -> String {
    match value {
        Value::Field(value) => field_value_to_string(value, db),
        Value::Record { table, record } => format!("record data {}:{}", table.name, record.id()),
    }
}

pub fn field_value_to_string(value: FieldValue, db: &Db) -> String {
    match value {
        FieldValue::Int(value) => value.to_string(),
        FieldValue::Bool(value) => value.to_string(),
        FieldValue::Timestamp(date_time) => DateTime::<Local>::from(date_time)
            .format("%d.%m.%Y %H:%M:%S")
            .to_string(),
        FieldValue::Text(value) => value,
        FieldValue::RecordId { id, table_name } => {
            let Some(table) = db.table(&table_name) else {
                return format!("<ERROR: table '{table_name}' not found>");
            };

            let display_field = table.main_display_field();

            if let Some(field) = display_field {
                let Some(value) = db.get(&table_name, id) else {
                    return format!("<ERROR: record '{table_name}:{id}' does not exist>");
                };

                let Some(field_value) = value.get_field(&field.value) else {
                    return format!("<ERROR: could not get field '{}' of '{table_name}:{id}'>", field.name);
                };

                field_value_to_string(field_value, db)
            } else {
                id_text(id)
            }
        }
    }
}
