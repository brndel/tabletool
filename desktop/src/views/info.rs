use chrono::{Datelike, Utc};
use dioxus::prelude::*;
use ui::{
    date_time_picker::DateTimePicker,
    modal_button::{ModalButton, ModalContent, ModalRoot},
};

const BLOG_CSS: Asset = asset!("/assets/blog.css");

#[component]
pub fn Info() -> Element {
    let mut date_time_1 = use_signal(|| Utc::now());
    let on_input_1 = move |v| date_time_1.set(v);
    let mut date_time_2 = use_signal(|| Utc::now().with_year(-10000).unwrap());
    let on_input_2 = move |v| date_time_2.set(v);

    let open_dialog = move |_| async move {
        dioxus::document::eval(r#" document.getElementById("info-dialog").showModal() "#)
            .await
            .unwrap();
    };

    rsx! {
        document::Link { rel: "stylesheet", href: BLOG_CSS}

        div {
            id: "blog",

            // Content
            h1 { "Heyy :3" }

            div {
                display: "flex",
                gap: "4pt",
                DateTimePicker { date_time: date_time_1(), on_input: on_input_1 }
            }
        }

        button {
            onclick: open_dialog,
            "Open",
        }

        dialog {
            id: "info-dialog",
            "closedby": "any",
            DateTimePicker { date_time: date_time_2(), on_input: on_input_2 }
        }

        ModalRoot {
            ModalButton { "Open" }
            ModalContent {
                header: "foobar",
                DateTimePicker { date_time: date_time_2(), on_input: on_input_2 }
            }
        }
    }
}
