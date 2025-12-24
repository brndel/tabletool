use db::Ulid;
use dioxus::{hooks::try_use_context, prelude::warn};

#[derive(Clone, Copy)]
pub struct ModalCtx {
    pub id: Ulid,
}

impl ModalCtx {
    pub async fn show_modal_ctx() {
        let Some(this) = try_use_context::<Self>() else {
            warn!("No context found");
            return;
        };

        if let Err(err) = dioxus::document::eval(&format!(
            r#" document.getElementById("{}").showModal() "#,
            this.id
        ))
        .await
        {
            warn!("Failed to run js eval with error {:?}", err)
        }
    }

    pub async fn close_ctx() {
        let Some(this) = try_use_context::<Self>() else {
            warn!("No context found");
            return;
        };

        if let Err(err) = dioxus::document::eval(&format!(
            r#" document.getElementById("{}").close() "#,
            this.id
        ))
        .await
        {
            warn!("Failed to run js eval with error {:?}", err)
        }
    }
}
