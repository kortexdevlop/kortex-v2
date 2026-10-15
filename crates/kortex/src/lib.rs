//! Facade used by tools: `kortex::contract!`, `#[kortex::engine]`, `kortex::runtime::*`.
pub use kortex_macros::{contract, engine};
pub use kortex_runtime as runtime;
