use std::sync::Arc;

use db::{Db, NamedTable, Table, TableField};
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

            db.register_table(NamedTable::new(
                "project_group",
                Table::new(
                    vec![TableField::new(name_field.clone(), db::FieldType::Text)],
                    Some(name_field.clone()),
                )
                .unwrap(),
            ));

            db.register_table(NamedTable::new(
                "project",
                Table::new(
                    vec![
                        TableField::new(name_field.clone(), db::FieldType::Text),
                        TableField::new(
                            "group",
                            db::FieldType::Record {
                                table_name: "project_group".into(),
                            },
                        ),
                    ],
                    Some(name_field),
                )
                .unwrap(),
            ));

            db.register_table(NamedTable::new(
                "work_time",
                Table::new(
                    vec![
                        TableField::new(
                            "project",
                            db::FieldType::Record {
                                table_name: "project".into(),
                            },
                        ),
                        TableField::new("notes", db::FieldType::Text),
                    ],
                    None,
                )
                .unwrap(),
            ));

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
