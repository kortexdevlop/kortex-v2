//! Kortex AI client logic shared by Mind and any future AI feature (MASTER_PROMPT §5.2): the
//! conversation wire format, streaming, tool definitions from contracts, the reference protocol
//! that keeps user data out of the model, plan validation and per-file consent. Pure Rust, no
//! network: hosts perform the HTTP call through `kortex-platform`.
//!
//! The model is Gemma 4 on Workers AI (ADR 0023). It never receives file or text contents unless
//! the user consents, per attachment; it names data with references (`@texte1`, `@fichier1`,
//! `@c1.text`) that [`plan`] resolves locally.
pub mod chat;
pub mod consent;
pub mod credits;
pub mod plan;
pub mod prompt;
pub mod stream;
pub mod tools;
