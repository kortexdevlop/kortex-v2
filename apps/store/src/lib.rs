//! Store application modules: pure catalogue rules and shared Dioxus presentation.
pub mod engine;
pub mod ui;
pub use engine::{Action, display_name, primary_action, universes, visible};
pub use ui::{Catalogue, STORE_CSS, Texts, ToolRow, text, texts};

#[cfg(not(target_arch = "wasm32"))]
pub mod delivery;
pub mod market;
