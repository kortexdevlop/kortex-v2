//! Seek's search rules live in the shared `kortex-search` crate (Mind uses the same engine to
//! pick candidate tools); this module keeps the application's own path to them.
pub use kortex_search::*;
