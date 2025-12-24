use crate::{
    button::{Button, ButtonProps, ButtonVariant},
    modal_button::ctx::ModalCtx,
};
use db::Ulid;
use dioxus::prelude::*;
use dioxus_free_icons::{icons::fa_solid_icons::FaX, Icon};

#[component]
pub fn ModalRoot(children: Element) -> Element {
    let id = use_hook(|| Ulid::new());

    use_context_provider(|| ModalCtx { id });

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("./style.css") }

        {children}
    }
}

#[component]
pub fn ModalButton(props: ButtonProps) -> Element {
    rsx! {
        Button {
            onclick: async |_| ModalCtx::show_modal_ctx().await,
            variant: props.variant,
            children: props.children,
        }
    }
}

#[component]
pub fn ModalContent(header: Option<String>, children: Element) -> Element {
    let ctx = use_context::<ModalCtx>();
    rsx! {
        dialog {
            class: "modal-dialog",
            id: "{ctx.id}",
            div {
                class: "modal-header",
                h1 {
                    {header}
                }
                ModalCloseButton {
                    Icon {
                        width: 12,
                        height: 12,
                        icon: FaX,
                    }
                }
            }
            {children}
        }
    }
}

#[component]
pub fn ModalCloseButton(onclick: Option<Callback<(), ()>>, children: Element) -> Element {
    rsx! {
        Button {
            onclick: move |_| async move {
                if let Some(onclick) = onclick {
                    onclick.call(());
                }

                ModalCtx::close_ctx().await
            },
            variant: ButtonVariant::Ghost,
            {children}
        }
    }
}
