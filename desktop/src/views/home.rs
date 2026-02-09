use std::sync::Arc;

use db::Db;
use db_core::{
    defs::table::{TableDef, TableFieldDef},
    named::Named,
    ty::FieldTy,
};
use dioxus::prelude::*;
use ui::{
    button::Button,
    table_tab_bar::{TableTab, TableTabBar},
    TableDialogButton,
};

use crate::Route;

#[component]
pub fn Home() -> Element {
    let db = use_context::<Db>();

    let mut reload_idx = use_signal(|| 0);

    let table_names = use_memo({
        let db = db.clone();
        move || {
            let _ = reload_idx();
            db.table_names()
        }
    });

    let on_submit = {
        let db = db.clone();
        move |table| {
            db.register_table(table);
            reload_idx.with_mut(|i| *i += 1);
        }
    };

    let mut init_db = {
        let db = db.clone();
        move || {
            let name_field = Arc::<str>::from("name");
            let project_group_name = Arc::<str>::from("project_group");
            let project_name = Arc::<str>::from("project");

            db.register_table(Named {
                name: project_group_name.clone(),
                value: TableDef {
                    fields: [(name_field.clone(), TableFieldDef { ty: FieldTy::Text })].into(),
                },
            });

            db.register_table(Named {
                name: project_name.clone(),
                value: TableDef {
                    fields: [
                        (name_field.clone(), TableFieldDef { ty: FieldTy::Text }),
                        (
                            "group".into(),
                            TableFieldDef {
                                ty: FieldTy::RecordId {
                                    table_name: project_group_name,
                                },
                            },
                        ),
                    ]
                    .into(),
                },
            });

            db.register_table(Named {
                name: "work_time".into(),
                value: TableDef {
                    fields: [
                        (
                            "project".into(),
                            TableFieldDef {
                                ty: FieldTy::RecordId {
                                    table_name: project_name,
                                },
                            },
                        ),
                        (
                            "start_time".into(),
                            TableFieldDef {
                                ty: FieldTy::Timestamp,
                            },
                        ),
                        (
                            "end_time".into(),
                            TableFieldDef {
                                ty: FieldTy::Timestamp,
                            },
                        ),
                        ("notes".into(), TableFieldDef { ty: FieldTy::Text }),
                    ]
                    .into(),
                },
            });

            reload_idx.with_mut(|x| *x += 1)
        }
    };

    rsx! {
        div {
            display: "flex",
            flex_direction: "row",
            gap: "0.5rem",
            align_items: "center",
            TableTabBar {
                for name in table_names.read().clone() {
                    TableTab {
                        to: Route::TablePage { name: name.to_string() },
                        "{name}"
                    }
                }
            }

            TableDialogButton { on_submit }
        }

        if table_names.with(|v| v.is_empty()) {
            Button {
                onclick: move |_| init_db(),
                "Init"
            }
        }
    }
}
