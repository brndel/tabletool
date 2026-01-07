use db_core::record::RecordBytes;
use dioxus::prelude::*;

use db::{Db, Ulid};

use ui::{
    alert_dialog::{
        AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
        AlertDialogRoot, AlertDialogTitle,
    },
    button::{Button, ButtonVariant},
    table_tab_bar::{TableTab, TableTabBar},
    DataTable, RecordDialogButton,
};

use crate::Route;

#[component]
pub fn TablePage(name: String) -> Element {
    let db = use_context::<Db>();

    let table_names = use_memo({
        let db = db.clone();
        move || db.table_names()
    });

    let mut table_name = use_signal(|| name.clone());

    let mut table_format = use_store({
        let db = db.clone();
        move || db.table(&table_name())
    });

    use_effect({
        let db = db.clone();
        use_reactive!(|name| {
            table_name.set(name);

            table_format.set(db.table(&table_name()))
        })
    });

    let table_format = table_format.transpose();

    let mut records = use_memo({
        let db = db.clone();
        move || db.get_all(&table_name()).unwrap_or_default()
    });

    let mut update_records = {
        let name = table_name.clone();
        let db = db.clone();

        move || {
            records.set(db.get_all(&name()).unwrap_or_default());
        }
    };

    let mut is_delete_table_dialog_open = use_signal(|| Some(false));

    let delete_table = {
        let db = db.clone();
        let nav = navigator();

        move || {
            let name = table_name.peek();
            db.delete_table(&name);
            nav.replace(Route::Home {});
        }
    };

    let insert_record = {
        let name = table_name.clone();
        let db = db.clone();
        let mut update_records = update_records.clone();

        move |record: RecordBytes| {
            db.insert_record(&name(), &record).unwrap();
            update_records();
        }
    };

    let delete_record = {
        let name = table_name.clone();
        let db = db.clone();
        let mut update_records = update_records.clone();

        move |id: Ulid| {
            db.delete_record(&name(), id).unwrap();

            update_records()
        }
    };

    rsx! {
        TableTabBar {
            for name in table_names.read().clone() {
                TableTab {
                    to: Route::TablePage { name: name.to_string() },
                    "{name}"
                }
            }
        }

        div {
            display: "flex",
            flex_direction: "row",
            gap: "0.5rem",

            // Button { onclick: move |_| update_records(), "Reload" }

            if let Some(table_format) = table_format {
                RecordDialogButton {
                    on_submit: insert_record,
                    table: table_format
                }
            }

            div {
                flex: "1"
            }

            Button { onclick: move |_| is_delete_table_dialog_open.set(Some(true)), variant: ButtonVariant::Destructive, "Delete Table" }
        }

        if let Some(table_format) = table_format {
            DataTable {
                items: records,
                delete: delete_record.clone(),
                table: table_format,
                table_name: table_name(),
            }
        } else {
            "Invalid Table"
        }

        AlertDialogRoot {
            open: is_delete_table_dialog_open,
            on_open_change: move |v| is_delete_table_dialog_open.set(Some(v)),
            AlertDialogContent {
                AlertDialogTitle { "Delete table" }
                AlertDialogDescription { "Are you sure you want to delete this table? This action cannot be undone." }
                AlertDialogContent {
                    AlertDialogCancel { "Cancel" }
                    AlertDialogAction { on_click:
                        move |_| {
                            delete_table()
                        }
                    ,
                    "Delete"
                    }
                }
            }
        }
    }
}
