//! Editor-facing analysis APIs.
//!
//! This module provides diagnostics, completions, hover, signature help, and
//! formatting as plain Rust functions. It is the stable boundary used by both
//! CLI commands and the LSP adapter, keeping JSON-RPC details out of analysis
//! code.

use serde::Serialize;

use mpl_hir::{
    Diagnostic,
    stdlib::{FUNCTIONS, Function, FunctionKind},
    validate,
};
use mpl_syntax::{SyntaxKind as TokenKind, SyntaxNode, TextRange, parse_syntax};

#[derive(Debug, Clone, Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hover {
    pub range: TextRange,
    pub contents: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignatureHelp {
    pub range: TextRange,
    pub signature: String,
}

pub fn diagnostics(input: &str) -> Vec<Diagnostic> {
    validate(&parse_syntax(input))
}

pub fn lints(input: &str) -> Vec<Diagnostic> {
    validate(&parse_syntax(input))
        .into_iter()
        .filter(|diag| !matches!(diag.severity, mpl_hir::Severity::Error))
        .collect()
}

pub fn format(input: &str) -> String {
    let parsed = parse_syntax(input);
    let tokens = tokens_from_syntax(parsed.syntax());
    format_tokens(&tokens)
}

pub fn completions(input: &str, offset: usize) -> Vec<CompletionItem> {
    let parsed = parse_syntax(input);
    let tokens = tokens_from_syntax(parsed.syntax());
    let offset = offset.min(input.len());

    if is_pipe_keyword_position(&tokens, offset) {
        return pipe_keyword_items();
    }

    if let Some(kind) = function_completion_kind(&tokens, offset) {
        return function_items(kind);
    }

    if is_after_comparison(&tokens, offset) {
        return literal_items();
    }

    if is_param_position(&tokens, offset) || is_interval_param_completion_position(&tokens, offset)
    {
        return vec![completion("$__interval", "parameter")];
    }

    if is_source_start(&tokens, offset) {
        return source_items();
    }

    Vec::new()
}

pub fn hover(input: &str, offset: usize) -> Option<Hover> {
    let parsed = parse_syntax(input);
    let tokens = tokens_from_syntax(parsed.syntax());
    let offset = offset.min(input.len());

    if let Some((function, range)) = function_at(&tokens, offset) {
        return Some(Hover {
            range,
            contents: format!("{}\n\n{}", function.signature, function.docs),
        });
    }

    let token = token_near(&tokens, offset)?;
    if let Some(contents) = keyword_docs(token) {
        return Some(Hover {
            range: token.range,
            contents: contents.to_string(),
        });
    }

    None
}

pub fn signature_help(input: &str, offset: usize) -> Option<SignatureHelp> {
    let parsed = parse_syntax(input);
    let tokens = tokens_from_syntax(parsed.syntax());
    let offset = offset.min(input.len());

    if let Some((function, range)) = function_for_signature(&tokens, offset) {
        return Some(SignatureHelp {
            range,
            signature: function.signature.to_string(),
        });
    }

    let token = token_near(&tokens, offset)?;
    keyword_signature(token).map(|signature| SignatureHelp {
        range: token.range,
        signature: signature.to_string(),
    })
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    text: String,
    range: TextRange,
}

fn tokens_from_syntax(root: &SyntaxNode) -> Vec<Token> {
    root.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .map(|token| {
            let range = token.text_range();
            Token {
                kind: token.kind(),
                text: token.text().to_string(),
                range: TextRange::new(
                    u32::from(range.start()) as usize,
                    u32::from(range.end()) as usize,
                ),
            }
        })
        .collect()
}

fn format_tokens(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut prev: Option<&Token> = None;

    for token in tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Whitespace)
    {
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

fn completion(label: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        detail: Some(detail.to_string()),
    }
}

fn pipe_keyword_items() -> Vec<CompletionItem> {
    [
        ("where", "filter rows"),
        ("filter", "deprecated filter alias"),
        ("map", "map function"),
        ("align", "align window"),
        ("group", "group series"),
        ("bucket", "bucket series"),
        ("extend", "derive field"),
        ("as", "alias source"),
    ]
    .into_iter()
    .map(|(label, detail)| completion(label, detail))
    .collect()
}

fn source_items() -> Vec<CompletionItem> {
    [
        ("from", "source query"),
        ("compute", "computed query"),
        ("set", "directive"),
    ]
    .into_iter()
    .map(|(label, detail)| completion(label, detail))
    .collect()
}

fn literal_items() -> Vec<CompletionItem> {
    [
        ("true", "boolean literal"),
        ("false", "boolean literal"),
        ("\"\"", "string literal"),
        ("#//", "regex literal"),
        ("0", "number literal"),
        ("$__interval", "parameter"),
    ]
    .into_iter()
    .map(|(label, detail)| completion(label, detail))
    .collect()
}

fn function_items(kind: FunctionKind) -> Vec<CompletionItem> {
    FUNCTIONS
        .iter()
        .filter(|function| function.kind == kind)
        .map(|function| completion(function.name, function.signature))
        .collect()
}

fn function_completion_kind(tokens: &[Token], offset: usize) -> Option<FunctionKind> {
    let cursor = completion_context_offset(tokens, offset);
    let Some(using) = previous_keyword_index(tokens, cursor, "using") else {
        return direct_map_function_kind(tokens, cursor);
    };
    if blocked_by_pipe_or_semicolon(tokens, using + 1, cursor) {
        return direct_map_function_kind(tokens, cursor);
    }

    previous_transform_kind(tokens, using)
}

fn direct_map_function_kind(tokens: &[Token], offset: usize) -> Option<FunctionKind> {
    let cursor = completion_context_offset(tokens, offset);
    let map = previous_keyword_index(tokens, cursor, "map")?;
    if blocked_by_pipe_or_semicolon(tokens, map + 1, cursor) {
        return None;
    }
    Some(FunctionKind::Map)
}

fn previous_transform_kind(tokens: &[Token], before: usize) -> Option<FunctionKind> {
    let mut index = before;
    while let Some(prev) = previous_meaningful_index(tokens, index) {
        let token = &tokens[prev];
        match token.kind {
            TokenKind::Pipe | TokenKind::Semicolon => return None,
            TokenKind::Keyword => match token.text.as_str() {
                "map" => return Some(FunctionKind::Map),
                "align" => return Some(FunctionKind::Align),
                "group" => return Some(FunctionKind::Group),
                "bucket" => return Some(FunctionKind::Bucket),
                "compute" => return Some(FunctionKind::Compute),
                _ => {}
            },
            _ => {}
        }
        index = prev;
    }
    None
}

fn is_pipe_keyword_position(tokens: &[Token], offset: usize) -> bool {
    let cursor = completion_context_offset(tokens, offset);
    previous_meaningful_token(tokens, cursor).is_some_and(|token| token.kind == TokenKind::Pipe)
}

fn is_after_comparison(tokens: &[Token], offset: usize) -> bool {
    let cursor = completion_context_offset(tokens, offset);
    previous_meaningful_token(tokens, cursor).is_some_and(|token| {
        token.kind == TokenKind::Cmp || (token.kind == TokenKind::Keyword && token.text == "is")
    })
}

fn is_param_position(tokens: &[Token], offset: usize) -> bool {
    token_at(tokens, offset).is_some_and(|token| token.kind == TokenKind::Param)
}

fn is_interval_param_completion_position(tokens: &[Token], offset: usize) -> bool {
    let cursor = completion_context_offset(tokens, offset);
    previous_meaningful_token(tokens, cursor).is_some_and(|token| match token.kind {
        TokenKind::Eq | TokenKind::Comma | TokenKind::LParen => true,
        TokenKind::Keyword => matches!(token.text.as_str(), "align" | "to"),
        _ => false,
    })
}

fn is_source_start(tokens: &[Token], offset: usize) -> bool {
    let cursor = completion_context_offset(tokens, offset);
    match previous_meaningful_token(tokens, cursor) {
        None => true,
        Some(token) => token.kind == TokenKind::Semicolon,
    }
}

fn completion_context_offset(tokens: &[Token], offset: usize) -> usize {
    token_at(tokens, offset)
        .filter(|token| {
            matches!(
                token.kind,
                TokenKind::Ident | TokenKind::Keyword | TokenKind::Param | TokenKind::EscapedIdent
            )
        })
        .map_or(offset, |token| token.range.start)
}

fn function_at(tokens: &[Token], offset: usize) -> Option<(&'static Function, TextRange)> {
    let token = token_near(tokens, offset)?;
    function_name_around(tokens, token.range.start)
        .and_then(|(name, range)| find_function(tokens, range.start, &name).map(|f| (f, range)))
}

fn function_for_signature(
    tokens: &[Token],
    offset: usize,
) -> Option<(&'static Function, TextRange)> {
    if let Some(function) = function_at(tokens, offset) {
        return Some(function);
    }

    let paren = containing_call_lparen(tokens, offset)?;
    let name = previous_name_start(tokens, paren)?;
    function_name_around(tokens, name)
        .and_then(|(name, range)| find_function(tokens, range.start, &name).map(|f| (f, range)))
}

fn function_name_around(tokens: &[Token], offset: usize) -> Option<(String, TextRange)> {
    let index = token_index_near(tokens, offset)?;
    let token = &tokens[index];
    if !is_name_part(token) && !is_operator(token) {
        return None;
    }

    if is_operator(token) {
        return Some((token.text.clone(), token.range));
    }

    let mut start = index;
    let mut end = index;

    if index >= 2
        && tokens[index - 1].kind == TokenKind::DoubleColon
        && is_name_part(&tokens[index - 2])
    {
        start = index - 2;
    }

    if index + 2 < tokens.len()
        && tokens[index + 1].kind == TokenKind::DoubleColon
        && is_name_part(&tokens[index + 2])
    {
        end = index + 2;
    }

    let mut name = String::new();
    for token in &tokens[start..=end] {
        name.push_str(&token.text);
    }

    Some((
        name,
        TextRange::new(tokens[start].range.start, tokens[end].range.end),
    ))
}

fn find_function(tokens: &[Token], offset: usize, name: &str) -> Option<&'static Function> {
    let kind = function_context_kind(tokens, offset);
    FUNCTIONS
        .iter()
        .find(|function| function.name == name && kind.is_some_and(|kind| function.kind == kind))
        .or_else(|| FUNCTIONS.iter().find(|function| function.name == name))
}

fn function_context_kind(tokens: &[Token], offset: usize) -> Option<FunctionKind> {
    let using = previous_keyword_index(tokens, offset, "using");
    let direct = direct_map_function_kind(tokens, offset);
    using
        .and_then(|using| previous_transform_kind(tokens, using))
        .or(direct)
}

fn containing_call_lparen(tokens: &[Token], offset: usize) -> Option<usize> {
    let cursor = completion_context_offset(tokens, offset);
    let mut depth = 0usize;
    let mut index = token_index_before(tokens, cursor)?;
    loop {
        let token = &tokens[index];
        match token.kind {
            TokenKind::RParen => depth += 1,
            TokenKind::LParen if depth == 0 => return Some(index),
            TokenKind::LParen => depth -= 1,
            TokenKind::Pipe | TokenKind::Semicolon => return None,
            _ => {}
        }

        let Some(prev) = previous_meaningful_index(tokens, index) else {
            return None;
        };
        index = prev;
    }
}

fn previous_name_start(tokens: &[Token], before: usize) -> Option<usize> {
    let prev = previous_meaningful_index(tokens, before)?;
    if is_name_part(&tokens[prev]) || is_operator(&tokens[prev]) {
        Some(tokens[prev].range.start)
    } else {
        None
    }
}

fn keyword_docs(token: &Token) -> Option<&'static str> {
    if token.kind == TokenKind::Pipe {
        return Some("Starts a pipeline transformation.");
    }

    if token.kind != TokenKind::Keyword {
        return None;
    }

    match token.text.as_str() {
        "set" => Some("Declares a query directive."),
        "from" => Some("Starts a source expression."),
        "where" => Some("Filters datapoints with a predicate."),
        "filter" => Some("Deprecated alias for `where`."),
        "map" => Some("Applies a map function to each datapoint."),
        "align" => Some("Aligns datapoints into fixed windows using an aggregate function."),
        "group" => Some("Groups series by tags using an aggregate function."),
        "bucket" => Some("Aggregates series into histogram buckets."),
        "extend" => Some("Adds derived fields."),
        "compute" => Some("Combines query results with a compute function."),
        "using" => Some("Introduces the function used by this transformation."),
        "to" => Some("Introduces an assignment target."),
        "by" => Some("Introduces grouping tags."),
        "as" => Some("Assigns an alias."),
        "and" => Some("Boolean conjunction."),
        "or" => Some("Boolean disjunction."),
        "not" => Some("Boolean negation."),
        "is" => Some("Checks a field type or literal match."),
        "true" | "false" => Some("Boolean literal."),
        _ => None,
    }
}

fn keyword_signature(token: &Token) -> Option<&'static str> {
    if token.kind != TokenKind::Keyword {
        return None;
    }

    match token.text.as_str() {
        "set" => Some("set <directive> = <value>"),
        "from" => Some("from <dataset>:<metric> [<range>] [as <alias>]"),
        "where" | "filter" => Some("where <field> <operator> <literal>"),
        "map" => Some("map [using] <function>"),
        "align" => Some("align <window> using <function>"),
        "group" => Some("group by <tag...> using <function>"),
        "bucket" => Some("bucket by <tag...> using <function>"),
        "extend" => Some("extend <name> = <expr>"),
        "compute" => Some("compute <name> using <function>"),
        "using" => Some("using <function>"),
        "as" => Some("as <alias>"),
        _ => None,
    }
}

fn token_near(tokens: &[Token], offset: usize) -> Option<&Token> {
    token_at(tokens, offset).or_else(|| previous_meaningful_token(tokens, offset))
}

fn token_at(tokens: &[Token], offset: usize) -> Option<&Token> {
    tokens
        .iter()
        .find(|token| !is_trivia(token) && token.range.start <= offset && offset <= token.range.end)
}

fn token_index_near(tokens: &[Token], offset: usize) -> Option<usize> {
    tokens
        .iter()
        .position(|token| {
            !is_trivia(token) && token.range.start <= offset && offset <= token.range.end
        })
        .or_else(|| token_index_before(tokens, offset))
}

fn token_index_before(tokens: &[Token], offset: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .rev()
        .find(|(_, token)| !is_trivia(token) && token.range.end <= offset)
        .map(|(index, _)| index)
}

fn previous_meaningful_token(tokens: &[Token], before: usize) -> Option<&Token> {
    token_index_before(tokens, before).map(|index| &tokens[index])
}

fn previous_meaningful_index(tokens: &[Token], before_index: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .take(before_index)
        .rev()
        .find(|(_, token)| !is_trivia(token))
        .map(|(index, _)| index)
}

fn previous_keyword_index(tokens: &[Token], before: usize, keyword: &str) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .rev()
        .find(|(_, token)| {
            !is_trivia(token)
                && token.range.end <= before
                && token.kind == TokenKind::Keyword
                && token.text == keyword
        })
        .map(|(index, _)| index)
}

fn blocked_by_pipe_or_semicolon(tokens: &[Token], start_index: usize, before: usize) -> bool {
    tokens.iter().skip(start_index).any(|token| {
        token.range.end <= before && matches!(token.kind, TokenKind::Pipe | TokenKind::Semicolon)
    })
}

fn is_name_part(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Ident | TokenKind::Keyword | TokenKind::EscapedIdent
    )
}

fn is_operator(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash
    )
}

fn is_trivia(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Whitespace | TokenKind::Comment | TokenKind::Eof
    )
}
