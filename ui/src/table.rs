
use bytepack::{ByteUnpacker, Unpack};
use chrono::{DateTime, Local, Utc};
use db::{Db, Ulid};
use db_core::{defs::table::{TableData, TableFieldData}, record::RecordBytes, ty::FieldTy};
use dioxus::prelude::*;

use crate::{
    components::{
        alert_dialog::{
            AlertDialogAction, AlertDialogActions, AlertDialogCancel, AlertDialogContent,
            AlertDialogDescription, AlertDialogRoot, AlertDialogTitle,
        },
        button::{Button, ButtonVariant},
    },
    id_card::{id_text, IdCard},
};

#[component]
pub fn DataTable(
    items: ReadSignal<Vec<RecordBytes>>,
    delete: Callback<Ulid, ()>,
    table: ReadSignal<TableData>,
    table_name: String,
) -> Element {
    let mut is_delete_dialog_open = use_signal(|| None);
    let mut selected_id = use_signal(|| None);

    let display_field = None;//table_v.main_display_field();

    let db = use_context::<Db>();

    rsx!(
        table {
            thead {
                tr {
                    th {"Id"}
                    for field_name in table().fields.keys() {
                        th {
                            Button {
                                onclick: {
                                    let has_index = false;//field.has_index;
                                    let field_name = field_name.clone();
                                    let db = db.clone();
                                    let table_name = table_name.clone();
                                    move |_| if has_index {
                                        let query_result = db.index_query(&format!("#{}:{}", table_name, field_name), Some(db::FieldValue::DateTime(Utc::now())), None);

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
                                if false { //field.has_index {
                                    "#"
                                }
                                "{field_name}"
                                if Some(field_name) == display_field {
                                    "*"
                                }
                            }
                        }
                    }
                    th {"…"}
                }
            }
            tbody {
                for record in items().iter() {
                    tr { key: "{record.id()}",
                        td {
                            IdCard {
                                id: record.id()
                            }
                        }
                        for field in table.read().fields.values() {
                            td {
                                "{extract_value(record, field, &db)}"
                            }
                        }
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
                            if let Some(id) = *selected_id.read() {
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

fn extract_value(
    record: &RecordBytes,
    field: &TableFieldData,
    db: &Db,
) -> String {
    let unpacker = ByteUnpacker::new(record.bytes());

    let s = unpack_to_string(&unpacker, field, db);

    s.unwrap_or_else(|| "???".to_owned())
}

pub fn unpack_to_string(
    unpacker: &ByteUnpacker,
    field: &TableFieldData,
    db: &Db,
) -> Option<String> {
    let offset = field.offset;

    match &field.ty {
        FieldTy::IntI32 => i32::unpack(offset, &unpacker).map(|x| x.to_string()),
        FieldTy::Timestamp => DateTime::<Utc>::unpack(offset, &unpacker).map(|dt| {
            DateTime::<Local>::from(dt)
                .format("%d.%m.%Y %H:%M:%S")
                .to_string()
        }),
        FieldTy::Bool => bool::unpack(offset, unpacker).map(|x| x.to_string()),
        FieldTy::Text => String::unpack(offset, &unpacker),
        FieldTy::RecordId { table_name } => {
            let id = Ulid::unpack(offset, &unpacker)?;

            // let table = db.table(&table_name).unwrap();
            // let format = table.packer_format();
            // let display_field = table.main_display_field().and_then(|name| {
            //     let ptr = format.field(name)?;
            //     let field = table.field(name)?;

            //     Some((ptr.pointer.offset, &field.ty))
            // });

            // let text_value = if let Some((offset, ty)) = display_field {
            //     let value = db.get(table_name, id)?;

            //     let unpacker = ByteUnpacker::new(value.bytes());
            //     let value = unpack_to_string(offset, &unpacker, ty, &db);

            //     value.unwrap_or_default()
            // } else {
            //     id_text(id)
            // };

            let text_value = id_text(id);

            Some(text_value)
        }
    }
}
