//! Executable entry point for the MPL language server.
//!
//! The binary delegates immediately to `mpl_lsp::run` so the protocol adapter
//! remains testable as a library while still producing a standalone LSP server
//! binary for editors.

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    mpl_lsp::run()
}
