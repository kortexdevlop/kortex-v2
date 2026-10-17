//! Store application modules: pure catalogue rules and shared Dioxus presentation.
pub mod engine;
pub mod ui;
pub use engine::{Action, display_name, primary_action, universes, visible};
pub use ui::{Catalogue, STORE_CSS, Texts, ToolRow, texts};

pub mod market;
