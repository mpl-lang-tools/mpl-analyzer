//! Shared text range primitives.
//!
//! Analyzer layers use byte-based text ranges internally so syntax, HIR, IDE,
//! CLI, and LSP conversions can exchange positions without depending on rowan
//! or LSP-specific range types.

use serde::Serialize;

pub type TextSize = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TextRange {
    pub start: TextSize,
    pub end: TextSize,
}

impl TextRange {
    pub const fn new(start: TextSize, end: TextSize) -> Self {
        Self { start, end }
    }

    pub const fn empty(offset: TextSize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    pub const fn contains(self, offset: TextSize) -> bool {
        self.start <= offset && offset <= self.end
    }
}
