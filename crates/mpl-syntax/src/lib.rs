//! Lossless syntax layer for MPL.
//!
//! This crate owns lexing, parsing, rowan CST integration, typed AST wrappers,
//! syntax diagnostics, text ranges, and syntax-aware formatting helpers. It is
//! intentionally independent of HIR, IDE, CLI, and LSP concerns.

pub mod format;
pub mod lexer;
pub mod parser;
pub mod syntax;
pub mod text;

pub use format::format_source;
pub use lexer::{Token, TokenKind, lex};
pub use parser::{Parse, SyntaxDiagnostic, parse, parse_syntax};
pub use syntax::{
    AssignmentNode, AstNode, ComputeRuleNode, DirectiveNode, ExprKind, ExprNode, FunctionCallNode,
    MplLanguage, NameRefNode, PipeKind, PipeNode, QueryNode, SourceFileNode, SourceNode,
    SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, ast_children, debug_tree, text_range,
    token_range, token_text,
};
pub use text::{TextRange, TextSize};
