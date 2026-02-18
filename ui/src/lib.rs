//! This crate contains all shared UI for the workspace.

mod components;
mod hero;
mod id_card;
mod navbar;
mod record_dialog;
mod table;
mod table_dialog;

pub use hero::Hero;
pub use id_card::IdCard;
pub use navbar::Navbar;
pub use record_dialog::RecordDialogButton;
pub use table::*;
pub use table_dialog::TableDialogButton;

pub use components::*;