use std::sync::Arc;

use bytepack::{ByteUnpacker, FieldWithPointer, PackFormat, PackPointer, Unpack};
use chrono::{DateTime, Local, Utc};
use db::{Db, FieldType, FieldValue, NumberFieldType, Table, TableField, Ulid};
use db_core::record::RecordBytes;
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
    table: ReadSignal<Table>,
    table_name: String,
) -> Element {
    let mut is_delete_dialog_open = use_signal(|| None);
    let mut selected_id = use_signal(|| None);

    let table_v = table();

    let display_field = table_v.main_display_field();
    let packer_format = table().packer_format();

    let db = use_context::<Db>();

    rsx!(
        table {
            thead {
                tr {
                    th {"Id"}
                    for field in table().fields().iter() {
                        th {
                            Button {
                                onclick: {
                                    let has_index = field.has_index;
                                    let field_name = field.name.clone();
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
                                if field.has_index {
                                    "#"
                                }
                                "{field.name()}"
                                if Some(field.name()) == display_field {
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
                        for (field, packer_field) in table.read().fields().iter().zip(packer_format.fields().iter()) {
                            td {
                                "{extract_value(record, field, packer_field, &db)}"
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
    field: &TableField,
    packer_field: &FieldWithPointer,
    db: &Db,
) -> String {
    let unpacker = ByteUnpacker::new(record.bytes());
    let offset = packer_field.pointer.offset;

    let s = unpack_to_string(offset, &unpacker, field.ty(), db);

    s.unwrap_or_else(|| "???".to_owned())
}

pub fn unpack_to_string(
    offset: u32,
    unpacker: &ByteUnpacker,
    ty: &FieldType,
    db: &Db,
) -> Option<String> {
    match ty {
        FieldType::Number(num) => match num {
            NumberFieldType::U8 => u8::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::U16 => u16::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::U32 => u32::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::U64 => u64::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::U128 => u128::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::I8 => i8::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::I16 => i16::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::I32 => i32::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::I64 => i64::unpack(offset, &unpacker).map(|x| x.to_string()),
            NumberFieldType::I128 => i128::unpack(offset, &unpacker).map(|x| x.to_string()),
        },
        FieldType::DateTime => DateTime::unpack(offset, &unpacker).map(|dt| {
            DateTime::<Local>::from(dt)
                .format("%d.%m.%Y %H:%M:%S")
                .to_string()
        }),
        FieldType::Text => String::unpack(offset, &unpacker),
        FieldType::Record { table_name } => {
            let id = Ulid::unpack(offset, &unpacker)?;

            let table = db.table(&table_name).unwrap();
            let format = table.packer_format();
            let display_field = table.main_display_field().and_then(|name| {
                let ptr = format.field(name)?;
                let field = table.field(name)?;

                Some((ptr.pointer.offset, &field.ty))
            });

            let text_value = if let Some((offset, ty)) = display_field {
                let value = db.get(table_name, id)?;

                let unpacker = ByteUnpacker::new(value.bytes());
                let value = unpack_to_string(offset, &unpacker, ty, &db);

                value.unwrap_or_default()
            } else {
                id_text(id)
            };

            Some(text_value)
        }
        FieldType::List { ty } => todo!(),
        FieldType::Option { ty } => {
            let ptr = PackPointer::unpack(offset, unpacker)?;
            if ptr == PackPointer::NULL {
                Some("<NULL>".to_string())
            } else {
                unpack_to_string(ptr.offset, unpacker, &ty, db)
            }
        }
    }
}
