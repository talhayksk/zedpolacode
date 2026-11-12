//! The engine behind Zed Polacode.
//!
//! This crate deliberately has no dependency on `zed_extension_api`, so the same
//! highlighter, themes and HTML renderer power both the Zed extension and the
//! standalone MCP server.

pub mod highlight;
pub mod options;
pub mod render;
pub mod theme;

pub use options::CardOptions;
