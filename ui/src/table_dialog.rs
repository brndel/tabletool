use db_core::{
    defs::table::{TableDef, TableFieldDef},
    named::Named,
    ty::FieldTy,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaEye, FaEyeSlash, FaHashtag},
    Icon,
};

use crate::{
    alert_dialog::{AlertDialogContent, AlertDialogDescription, AlertDialogRoot, AlertDialogTitle},
    components::{
        button::{Button, ButtonVariant},
        input::Input,
        select::{
            Select, SelectGroup, SelectGroupLabel, SelectItemIndicator, SelectList, SelectOption,
            SelectTrigger, SelectValue,
        },
    },
};

#[component]
pub fn TableDialogButton(on_submit: Callback<Named<TableDef>, ()>) -> Element {
    let mut open: Signal<bool> = use_signal(|| false);

    let mut name = use_signal(|| String::new());
    let mut fields = use_store(|| Vec::<FieldStore>::new());
    let mut main_display_field_idx = use_signal(|| Option::<usize>::None);

    let mut onsubmit_form = move || {
        let name = name.peek();
        let fields = fields.peek();

        let fields = fields
            .iter()
            .map(|field| (field.name.clone().into(), TableFieldDef { ty: field.ty.clone() }))
            .collect();

        // let main_display_field_name =
        //     main_display_field_idx().and_then(|idx| Some(fields.get(idx)?.name.clone()));

        let table = TableDef { fields };

        on_submit(Named {
            name: name.clone().into(),
            value: table,
        });
        
        open.set(false);
        // match Table::new(fields, main_display_field_name) {
        //     Ok(table) => {
        //     }
        //     Err(err) => {
        //         error!("couldnt create table {:?}", err);
        //     }
        // }
    };

    rsx! {
        Button {
            r#type: "button",
            "data-style": "outline",
            onclick: move |_| open.set(true),
            "New Table"
        }
        AlertDialogRoot { open: open(), on_open_change: move |v| open.set(v),
            AlertDialogContent {
                AlertDialogTitle { "New Table" }
                AlertDialogDescription { "Create a new table" }
                div {
                    display: "flex",
                    flex_direction: "column",
                    gap: ".5rem",

                    div {
                        display: "flex",
                        flex_direction: "row",
                        gap: "0.5rem",

                        Input {
                            placeholder: "name",
                            value: "{name()}",
                            autocorrect: "off",
                            flex: "1",
                            oninput: move |ev: Event<FormData>| name.set(ev.value())
                        }

                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |ev: Event<MouseData>| {
                                ev.prevent_default();
                                fields.push(FieldStore { name: String::new(), ty: FieldTy::Text, has_index: false });
                            },
                            "New Field"
                        }
                    }

                    Button {
                        onclick: move |_| main_display_field_idx.set(None),
                        variant: if main_display_field_idx() == None { ButtonVariant::Ghost } else { ButtonVariant::Secondary },
                        Icon {
                            width: 12,
                            height: 12,
                            icon: FaEyeSlash,
                        }
                        "No Display Field"
                    }

                    for (idx, field) in fields.iter().enumerate() {
                        div {
                            key: "{idx}",
                            display: "flex",
                            flex_direction: "row",
                            gap: ".5rem",
                            Button {
                                onclick: move |_| main_display_field_idx.set(Some(idx)),
                                variant: if main_display_field_idx() == Some(idx) { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                                Icon {
                                    width: 12,
                                    height: 12,
                                    icon: FaEye,
                                }
                            }
                            Button {
                                onclick: move |_| field.has_index().toggle(),
                                variant: if field.has_index()() { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                                Icon {
                                    width: 12,
                                    height: 12,
                                    icon: FaHashtag,
                                }
                            }
                            Input { placeholder: "Name", value: "{field.name()}", autocorrect: "off", flex: "1", oninput: {let mut name = field.name(); move |ev: Event<FormData>| {name.set(ev.value())}} }
                            FieldTypeSelect { value: field.ty() }
                        }
                    }



                    Button {
                        onclick: move |_| onsubmit_form(),
                        "Create Table",
                    }
                }
            }
        }
    }
}

#[derive(Store)]
struct FieldStore {
    name: String,
    ty: FieldTy,
    has_index: bool,
}

#[component]
pub fn FieldTypeSelect(value: Store<FieldTy>) -> Element {
    let mut picker_value = use_signal(|| Some(Some(value())));

    use_effect(move || {
        let picker_value = picker_value.read();

        if let Some(Some(ty)) = picker_value.clone() {
            value.set(ty);
        }
    });

    let numbers = [
        FieldTy::IntI32,
        // FieldType::Number(NumberFieldType::U16),
        // FieldType::Number(NumberFieldType::U32),
        // FieldType::Number(NumberFieldType::U64),
        // FieldType::Number(NumberFieldType::I8),
        // FieldType::Number(NumberFieldType::I16),
        // FieldType::Number(NumberFieldType::I32),
        // FieldType::Number(NumberFieldType::I64),
        // FieldType::Number(NumberFieldType::I128),
    ];

    let number_options = numbers.iter().enumerate().map(|(idx, value)| {
        rsx! {
            SelectOption::<FieldTy> {
                index: idx,
                value: value.clone(),
                text_value: "{value:?}",
                "{value:?}"
                SelectItemIndicator {}
            }
        }
    });

    if let FieldTy::RecordId { table_name } = value() {
        rsx! {
            Input {
                value: "{table_name}",
                autocorrect: "off",
                oninput: move |ev: Event<FormData>| value.set(FieldTy::RecordId {table_name: ev.value().into()})
            }
        }
    } else {
        rsx! {
            Select::<FieldTy> { placeholder: "Select a type",
                value: picker_value,
                on_value_change: move |v| picker_value.set(Some(v)),
                SelectTrigger { aria_label: "Select Trigger", width: "12rem", SelectValue {} }
                SelectList { aria_label: "Select Demo",
                    SelectGroup {
                        SelectGroupLabel { "Numbers" }
                        {number_options}
                    }
                    SelectGroup {
                        SelectGroupLabel { "Other" }
                        SelectOption::<FieldTy> {
                            index: numbers.len(),
                            value: FieldTy::Text,
                            text_value: "Text",
                            "Text"
                            SelectItemIndicator {}
                        }
                        SelectOption::<FieldTy> {
                            index: numbers.len() + 1,
                            value: FieldTy::Timestamp,
                            text_value: "Timestamp",
                            "Timestamp"
                            SelectItemIndicator {}
                        }
                    }
                    SelectGroup {
                        SelectGroupLabel { "Record" }
                        SelectOption::<FieldTy> {
                            index: numbers.len() + 2,
                            value: FieldTy::RecordId { table_name: String::new().into() },
                            text_value: "Record",
                            "Record"
                            SelectItemIndicator {}
                        }
                    }
                }
            }
        }
    }
}
