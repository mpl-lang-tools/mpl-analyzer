//! IDE layer for MPL editor features.
//!
//! The crate exposes editor-oriented APIs that combine syntax and HIR without
//! depending on LSP wire types. This keeps feature behavior testable in-process
//! and reusable by multiple frontends.

pub mod analysis;

pub use analysis::*;
