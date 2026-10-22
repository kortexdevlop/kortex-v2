//! The shared player (ADR 0035): the window (desktop) or page (web) of any tool whose interface
//! is generated from its contract. One build per platform serves every tool: the tool arrives as
//! a payload (`tool.toml`, locales, glyph, `engine.wasm`), never as compiled code.
pub mod tool;

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "web")]
pub mod web;

/// Design tokens and component styles, embedded like in the apps.
pub const CSS: &str = include_str!("../../../design/dist/kortex.css");
