//! Semantic layer for MPL.
//!
//! This crate lowers lossless syntax into explicit HIR data structures and owns
//! semantic diagnostics. Keeping this layer independent from IDE and LSP types
//! lets editor features share one semantic model while the syntax crate remains
//! purely about parsing and source structure.

pub mod lower;
pub mod model;
pub mod stdlib;
pub mod validate;

pub use lower::lower;
pub use model::*;
pub use validate::{Diagnostic, Severity, validate};
