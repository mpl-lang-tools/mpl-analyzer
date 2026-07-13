//! Syntax-preserving formatter entry point.
//!
//! The formatter currently works from lexer tokens and gives the IDE and CLI a
//! deterministic formatting path. It lives in `mpl-syntax` because formatting
//! must preserve source trivia and should not depend on semantic analysis.

use crate::lexer::{Token, TokenKind, lex};

pub fn format_source(input: &str) -> String {
    let tokens = lex(input);
    let mut out = String::new();
    let mut prev: Option<&Token> = None;

    for token in tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Whitespace)
    {
        if token.kind == TokenKind::Eof {
            break;
        }

        if token.kind == TokenKind::Comment {
            if !at_line_start(&out) {
                trim_spaces(&mut out);
                out.push(' ');
            }
            out.push_str(token.text.trim_end());
            out.push('\n');
            prev = Some(token);
            continue;
        }

        if token.kind == TokenKind::Pipe {
            trim_spaces(&mut out);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("| ");
            prev = Some(token);
            continue;
        }

        if token.kind == TokenKind::Semicolon {
            trim_spaces(&mut out);
            out.push(';');
            out.push('\n');
            prev = Some(token);
            continue;
        }

        if needs_space(prev, token, &out) {
            out.push(' ');
        }

        match token.kind {
            TokenKind::Comma => {
                trim_spaces(&mut out);
                out.push_str(", ");
            }
            TokenKind::Colon | TokenKind::DoubleColon | TokenKind::Dot | TokenKind::DotDot => {
                trim_spaces(&mut out);
                out.push_str(&token.text);
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                trim_spaces(&mut out);
                out.push_str(&token.text);
            }
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                out.push_str(&token.text);
            }
            TokenKind::Eq
            | TokenKind::Cmp
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash => {
                trim_spaces(&mut out);
                if !at_line_start(&out) {
                    out.push(' ');
                }
                out.push_str(&token.text);
                out.push(' ');
            }
            _ => out.push_str(&token.text),
        }

        prev = Some(token);
    }

    trim_blank_lines(&mut out);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn needs_space(prev: Option<&Token>, token: &Token, out: &str) -> bool {
    if out.is_empty() || out.ends_with([' ', '\n', '(', '[', '{', ':', '.']) {
        return false;
    }

    let Some(prev) = prev else {
        return false;
    };

    if matches!(
        token.kind,
        TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::DoubleColon
            | TokenKind::Dot
            | TokenKind::DotDot
            | TokenKind::LBracket
            | TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace
            | TokenKind::Eq
            | TokenKind::Cmp
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
    ) {
        return false;
    }

    if token.kind == TokenKind::LParen {
        return matches!(
            prev.kind,
            TokenKind::Keyword
                if matches!(
                    prev.text.as_str(),
                    "where" | "filter" | "not" | "and" | "or" | "is" | "by" | "to" | "using" | "as"
                )
        );
    }

    if matches!(
        prev.kind,
        TokenKind::Colon
            | TokenKind::DoubleColon
            | TokenKind::Dot
            | TokenKind::DotDot
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::LBrace
            | TokenKind::Pipe
            | TokenKind::Eq
            | TokenKind::Cmp
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
    ) {
        return false;
    }

    true
}

fn at_line_start(out: &str) -> bool {
    out.rsplit_once('\n')
        .map(|(_, tail)| tail.trim().is_empty())
        .unwrap_or_else(|| out.trim().is_empty())
}

fn trim_spaces(out: &mut String) {
    while out.ends_with(' ') || out.ends_with('\t') {
        out.pop();
    }
}

fn trim_blank_lines(out: &mut String) {
    while out.ends_with("\n\n") {
        out.pop();
    }
}
