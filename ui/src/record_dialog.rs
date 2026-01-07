use std::{str::FromStr, sync::Arc};

use bytepack::{BytePacker, ByteUnpacker, PackFormat};
use chrono::{DateTime, Timelike, Utc};
use db::{Db, FieldType, FieldValue, NumberFieldType, Table, Ulid};
use db_core::record::RecordBytes;
use dioxus::prelude::*;

use crate::{
    button::ButtonVariant,
    components::{
        input::Input,
        label::Label,
    },
    date_time_picker::DateTimePicker,
    id_card::id_text,
    modal_button::{ModalButton, ModalCloseButton, ModalContent, ModalRoot},
    select::{
        Select, SelectGroup, SelectGroupLabel, SelectItemIndicator, SelectList, SelectOption,
        SelectTrigger, SelectValue,
    }, table::unpack_to_string,
};

#[component]
pub fn RecordDialogButton(
    on_submit: Callback<RecordBytes, ()>,
    table: ReadSignal<Table>,
) -> Element {
    let mut open: Signal<bool> = use_signal(|| false);

    let mut input_values = use_store(|| {
        println!("init store");
        Vec::new()
    });

    let mut input_values_effect = use_effect(move || {
        println!("setting input_values");
        input_values.set(
            table()
                .fields()
                .iter()
                .map(|field| RecordField::new(field.name().to_owned(), field.ty().clone()))
                .collect::<Vec<_>>(),
        );
    });

    let mut onsubmit_button = move || {
        let values = input_values.peek();
        let table = table.peek();
        let format = table.packer_format();

        let mut packer = BytePacker::new(format.fixed_byte_count());
        let mut fields = packer.fields(&format, 0);

        for field in values.iter() {
            if let Some(value) = &field.value() {
                fields.pack(&field.name, value);
            } else {
                warn!("field {} has no value", field.name);
                return;
            }
        }

        let bytes = packer.finish();

        on_submit(RecordBytes::create(bytes));
        open.set(false);
        input_values_effect.mark_dirty();
    };

    rsx! {
        ModalRoot {
            ModalButton {
                variant: ButtonVariant::Secondary,
                "New Row"
            }
            ModalContent {
                header: "Add Record",
                div {
                    display: "flex",
                    flex_direction: "column",
                    gap: ".5rem",

                    for field in input_values.iter() {
                        RecordFieldInput { field: field }
                    }

                    ModalCloseButton {
                        onclick: move |_| onsubmit_button(),
                        "Add",
                    }
                }
            }
        }
    }
}

#[component]
fn RecordFieldInput(field: Store<RecordField>) -> Element {
    let name = field.name();

    let oninput = move |ev: Event<FormData>| {
        field.with_mut(move |field| match &mut field.value {
            RecordFieldValue::StringField(field) => field.update(ev.value()),
            _ => (),
        });
    };

    let db = use_context::<Db>();

    field.value().with(|value| {
        match value {
            RecordFieldValue::DateTime(date) => {
                let date = *date;
                rsx! {
                    Label { key: "{name()}-label", html_for: "{name()}", "{name()}" }
                    DateTimePicker {
                        key: "{name()}-input",
                        date_time: date,
                        on_input: move |date| field.with_mut(|field| field.value = RecordFieldValue::DateTime(date))
                    }
                }
            },
            RecordFieldValue::Text(text) => rsx! {
                Label { key: "{name()}-label", html_for: "{name()}", "{name()}" }

                Input { key: "{name()}-input", id: "{name()}", placeholder: "{name()}", value: "{text}", oninput: move |ev: Event<FormData>| { field.with_mut(|field| field.value = RecordFieldValue::Text(ev.value())) } }
            },
            RecordFieldValue::StringField(string_field) => rsx! {
                Label { key: "{name()}-label", html_for: "{name()}", "{name()}" }

                Input { class: if string_field.value.is_err() {"input invalid"} else {"input"}, key: "{name()}-input", id: "{name()}", placeholder: "{name()}", value: "{string_field.string}", oninput}
            },
            RecordFieldValue::Record { table_name, id} => {
                let db = use_context::<Db>();

                let table = db.table(&table_name).unwrap();
                let format = table.packer_format();
                let display_field = table.main_display_field().and_then(|name| {
                    let ptr = format.field(name)?;
                    let field = table.field(name)?;

                    Some((ptr.pointer.offset, &field.ty))
                });

                let records = db.get_all(&table_name).unwrap();


                let options = records.iter().enumerate().map(|(idx, value)| {
                    let text_value = if let Some((offset, ty)) = display_field {
                        let unpacker = ByteUnpacker::new(value.bytes());
                        let value = unpack_to_string(offset, &unpacker, ty, &db);
                        // unpacker.unpack("name")
                        value.unwrap_or_default()
                    } else {
                        id_text(value.id())
                    };
                    rsx! {
                        SelectOption::<Ulid> {
                            index: idx,
                            value: value.id(),
                            text_value: "{text_value}",
                            "{text_value}"
                            SelectItemIndicator {}
                        }
                    }
                });

                rsx! {
                    Label { key: "{name()}-label", html_for: "{name()}", "{name()}" }
                Select::<Ulid> { placeholder: "Select record",
                    value: id.clone(),
                    on_value_change: move |v| {
                        field.value().with_mut(|value| {
                            match value {
                                RecordFieldValue::Record { id, .. } => *id = v,
                                _ => ()
                            }
                        })
                    },

                    SelectTrigger { aria_label: "Select Trigger", width: "12rem", SelectValue {} }
                    SelectList { aria_label: "Select Record",
                        SelectGroup {
                            SelectGroupLabel { "Records" }
                            {options}
                        }
                    }
                }
                // Input { class: if string_field.value.is_err() {"input invalid"} else {"input"}, key: "{name()}-input", id: "{name()}", placeholder: "{name()}", value: "{string_field.string}", oninput}
                }
            },
        }
    })
}

#[derive(Store)]
struct RecordField {
    name: String,
    value: RecordFieldValue,
}

#[derive(Store)]
enum RecordFieldValue {
    DateTime(DateTime<Utc>),
    Text(String),
    StringField(RecordStringField),
    Record {
        table_name: Arc<str>,
        id: Option<Ulid>,
    },
}

struct RecordStringField {
    ty: StringFieldType,
    string: String,
    value: Result<FieldValue, String>,
}

enum StringFieldType {
    Number(NumberFieldType),
}

impl RecordField {
    pub fn new(name: String, ty: FieldType) -> Self {
        Self {
            name,
            value: match ty {
                FieldType::Number(num) => RecordFieldValue::StringField(RecordStringField::new(
                    String::new(),
                    StringFieldType::Number(num),
                )),
                FieldType::DateTime => RecordFieldValue::DateTime(Utc::now().with_second(0).unwrap().with_nanosecond(0).unwrap()),
                FieldType::Text => RecordFieldValue::Text(String::new()),
                FieldType::Record { table_name } => RecordFieldValue::Record {
                    table_name,
                    id: None,
                },
                FieldType::List { ty } => todo!(),
                FieldType::Option { ty } => todo!(),
            },
        }
    }

    pub fn value(&self) -> Option<FieldValue> {
        match &self.value {
            RecordFieldValue::DateTime(date_time) => Some(FieldValue::DateTime(*date_time)),
            RecordFieldValue::Text(text) => Some(FieldValue::Text(text.clone())),
            RecordFieldValue::StringField(field) => field.value.as_ref().ok().cloned(),
            RecordFieldValue::Record { id, .. } => id.map(|id| FieldValue::RecordId(id)),
        }
    }
}

impl RecordStringField {
    pub fn new(string: String, ty: StringFieldType) -> Self {
        let mut this = Self {
            ty,
            string: string.clone(),
            value: Err(String::new()),
        };

        this.update(string);

        this
    }

    pub fn update(&mut self, string: String) {
        self.string = string;

        let value = match &self.ty {
            StringFieldType::Number(num) => match num {
                NumberFieldType::U8 => u8::from_str(&self.string).map(FieldValue::U8),
                NumberFieldType::U16 => u16::from_str(&self.string).map(FieldValue::U16),
                NumberFieldType::U32 => u32::from_str(&self.string).map(FieldValue::U32),
                NumberFieldType::U64 => u64::from_str(&self.string).map(FieldValue::U64),
                NumberFieldType::U128 => u128::from_str(&self.string).map(FieldValue::U128),
                NumberFieldType::I8 => i8::from_str(&self.string).map(FieldValue::I8),
                NumberFieldType::I16 => i16::from_str(&self.string).map(FieldValue::I16),
                NumberFieldType::I32 => i32::from_str(&self.string).map(FieldValue::I32),
                NumberFieldType::I64 => i64::from_str(&self.string).map(FieldValue::I64),
                NumberFieldType::I128 => i128::from_str(&self.string).map(FieldValue::I128),
            }
            .map_err(|err| format!("{:?}", err)),
        };

        self.value = value;
    }
}
