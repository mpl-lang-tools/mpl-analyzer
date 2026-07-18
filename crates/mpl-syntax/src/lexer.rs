//! MPL lexer.
//!
//! The lexer converts source text into ranged tokens while preserving trivia,
//! comments, unknown text, and exact byte spans. Parsing, formatting, and IDE
//! range conversion all rely on this lossless token stream.

use serde::Serialize;

use crate::text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Whitespace,
    Comment,
    Ident,
    EscapedIdent,
    Param,
    String,
    Regex,
    Number,
    Duration,
    Timestamp,
    Bool,
    Pipe,
    Colon,
    DoubleColon,
    Comma,
    Semicolon,
    Dot,
    DotDot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eq,
    Cmp,
    Plus,
    Minus,
    Star,
    Slash,
    Keyword,
    Unknown,
    Eof,
}

#[derive(Debug, Clone, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub range: TextRange,
}

pub fn lex(input: &str) -> Vec<Token> {
    let mut lexer = Lexer { input, pos: 0 };
    let mut out = Vec::new();
    while lexer.pos < input.len() {
        out.push(lexer.next_token());
    }
    out.push(Token {
        kind: TokenKind::Eof,
        text: String::new(),
        range: TextRange::empty(input.len()),
    });
    out
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl Lexer<'_> {
    fn next_token(&mut self) -> Token {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        let b = bytes[start];
        let kind = match b {
            b' ' | b'\t' | b'\r' | b'\n' => {
                self.bump_while(|c| c.is_ascii_whitespace());
                TokenKind::Whitespace
            }
            b'/' if self.peek(1) == Some(b'/') => {
                self.pos += 2;
                self.bump_while(|c| c != b'\n');
                TokenKind::Comment
            }
            b'`' => {
                self.bump_escaped_ident();
                TokenKind::EscapedIdent
            }
            b'$' => {
                self.pos += 1;
                if self.peek(0) == Some(b'`') {
                    self.bump_escaped_ident();
                } else {
                    self.bump_ident_tail();
                }
                TokenKind::Param
            }
            b'"' => {
                self.bump_string();
                TokenKind::String
            }
            b'#' if self.peek(1) == Some(b'/') || self.peek(1) == Some(b's') => {
                self.bump_regex();
                TokenKind::Regex
            }
            b'0'..=b'9' => self.bump_number_like(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.bump_word(),
            b'|' => {
                self.pos += 1;
                TokenKind::Pipe
            }
            b':' if self.peek(1) == Some(b':') => {
                self.pos += 2;
                TokenKind::DoubleColon
            }
            b':' => {
                self.pos += 1;
                TokenKind::Colon
            }
            b',' => {
                self.pos += 1;
                TokenKind::Comma
            }
            b';' => {
                self.pos += 1;
                TokenKind::Semicolon
            }
            b'.' if self.peek(1) == Some(b'.') => {
                self.pos += 2;
                TokenKind::DotDot
            }
            b'.' => {
                self.pos += 1;
                TokenKind::Dot
            }
            b'(' => {
                self.pos += 1;
                TokenKind::LParen
            }
            b')' => {
                self.pos += 1;
                TokenKind::RParen
            }
            b'{' => {
                self.pos += 1;
                TokenKind::LBrace
            }
            b'}' => {
                self.pos += 1;
                TokenKind::RBrace
            }
            b'[' => {
                self.pos += 1;
                TokenKind::LBracket
            }
            b']' => {
                self.pos += 1;
                TokenKind::RBracket
            }
            b'=' if self.peek(1) == Some(b'=') => {
                self.pos += 2;
                TokenKind::Cmp
            }
            b'=' if self.peek(1) == Some(b'~') => {
                self.pos += 2;
                TokenKind::Cmp
            }
            b'!' if self.peek(1) == Some(b'=') => {
                self.pos += 2;
                TokenKind::Cmp
            }
            b'!' if self.peek(1) == Some(b'~') => {
                self.pos += 2;
                TokenKind::Cmp
            }
            b'<' | b'>' => {
                self.pos += 1;
                if self.peek(0) == Some(b'=') {
                    self.pos += 1;
                }
                TokenKind::Cmp
            }
            b'=' => {
                self.pos += 1;
                TokenKind::Eq
            }
            b'+' => {
                self.pos += 1;
                if self.peek_word(0, b"inf") {
                    self.pos += 3;
                    TokenKind::Number
                } else if self.peek(0).is_some_and(|c| c.is_ascii_digit()) {
                    self.bump_number_tail();
                    classify_number(&self.input[start..self.pos])
                } else {
                    TokenKind::Plus
                }
            }
            b'-' => {
                self.pos += 1;
                if self.peek_word(0, b"inf") {
                    self.pos += 3;
                    TokenKind::Number
                } else if self.peek(0).is_some_and(|c| c.is_ascii_digit()) {
                    self.bump_number_tail();
                    classify_number(&self.input[start..self.pos])
                } else {
                    TokenKind::Minus
                }
            }
            b'*' => {
                self.pos += 1;
                TokenKind::Star
            }
            b'/' => {
                self.pos += 1;
                TokenKind::Slash
            }
            _ => {
                self.pos += 1;
                TokenKind::Unknown
            }
        };
        Token {
            kind,
            text: self.input[start..self.pos].to_string(),
            range: TextRange::new(start, self.pos),
        }
    }

    fn peek(&self, offset: usize) -> Option<u8> {
        self.input.as_bytes().get(self.pos + offset).copied()
    }

    fn peek_word(&self, offset: usize, word: &[u8]) -> bool {
        self.input
            .as_bytes()
            .get(self.pos + offset..self.pos + offset + word.len())
            == Some(word)
    }

    fn bump_while(&mut self, mut pred: impl FnMut(u8) -> bool) {
        while self.pos < self.input.len() && pred(self.input.as_bytes()[self.pos]) {
            self.pos += 1;
        }
    }

    fn bump_ident_tail(&mut self) {
        self.bump_while(|c| c.is_ascii_alphanumeric() || c == b'_');
    }

    fn bump_escaped_ident(&mut self) {
        if self.peek(0) == Some(b'`') {
            self.pos += 1;
        }
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b'\\' => self.pos = (self.pos + 2).min(self.input.len()),
                b'`' => {
                    self.pos += 1;
                    break;
                }
                _ => self.pos += 1,
            }
        }
    }

    fn bump_string(&mut self) {
        self.pos += 1;
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b'\\' => self.pos = (self.pos + 2).min(self.input.len()),
                b'"' => {
                    self.pos += 1;
                    break;
                }
                _ => self.pos += 1,
            }
        }
    }

    fn bump_regex(&mut self) {
        self.pos += if self.peek(1) == Some(b's') { 3 } else { 2 };
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b'\\' => self.pos = (self.pos + 2).min(self.input.len()),
                b'/' => {
                    self.pos += 1;
                    if self.input.as_bytes()[self.pos.saturating_sub(2)] == b's' {
                        continue;
                    }
                    break;
                }
                _ => self.pos += 1,
            }
        }
    }

    fn bump_word(&mut self) -> TokenKind {
        self.pos += 1;
        self.bump_ident_tail();
        match &self.input[self.pos
            - (self.input[..self.pos]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .count())..self.pos]
        {
            "true" | "false" => TokenKind::Bool,
            "set" | "param" | "where" | "filter" | "map" | "align" | "group" | "bucket"
            | "extend" | "compute" | "using" | "to" | "over" | "by" | "as" | "ifdef" | "else"
            | "and" | "or" | "not" | "is" | "from" | "sample" => TokenKind::Keyword,
            "inf" => TokenKind::Number,
            _ => TokenKind::Ident,
        }
    }

    fn bump_number_like(&mut self) -> TokenKind {
        self.pos += 1;
        self.bump_number_tail();
        classify_number(
            &self.input[self.pos
                - self.input[..self.pos]
                    .chars()
                    .rev()
                    .take_while(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '+' | 'T' | 'Z')
                    })
                    .count()..self.pos],
        )
    }

    fn bump_number_tail(&mut self) {
        self.bump_while(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b':' | b'-' | b'+' | b'T' | b'Z')
        });
    }
}

fn classify_number(text: &str) -> TokenKind {
    if text.contains('T') {
        TokenKind::Timestamp
    } else if text.ends_with("ms")
        || text.ends_with('s')
        || text.ends_with('m')
        || text.ends_with('h')
        || text.ends_with('d')
        || text.ends_with('w')
        || text.ends_with('M')
        || text.ends_with('y')
    {
        TokenKind::Duration
    } else {
        TokenKind::Number
    }
}
