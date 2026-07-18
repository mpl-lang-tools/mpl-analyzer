//! Lossless MPL parser.
//!
//! The parser consumes lexer tokens, reports recoverable syntax diagnostics, and
//! builds a rowan CST directly. Parse routines return only parser-control data
//! such as source ranges; the CST plus typed wrappers are the public syntax
//! representation.

use crate::lexer::{Token, TokenKind, lex};
use crate::syntax::{AstNode, SourceFileNode, SyntaxKind, SyntaxNode};
use crate::text::TextRange;

#[derive(Debug, Clone)]
pub struct Parse<T = SourceFileNode> {
    tree: T,
    diagnostics: Vec<SyntaxDiagnostic>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyntaxDiagnostic {
    pub message: String,
    pub range: TextRange,
}

pub fn parse(input: &str) -> Parse<SourceFileNode> {
    parse_syntax(input)
}

pub fn parse_syntax(input: &str) -> Parse<SourceFileNode> {
    let Parsed {
        syntax,
        diagnostics,
    } = parse_impl(input);
    let tree = SourceFileNode::cast(syntax).expect("parser always produces a root node");
    Parse { tree, diagnostics }
}

impl<T> Parse<T> {
    pub fn tree(&self) -> &T {
        &self.tree
    }

    pub fn into_tree(self) -> T {
        self.tree
    }

    pub fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        &self.diagnostics
    }
}

impl Parse<SourceFileNode> {
    pub fn syntax(&self) -> &SyntaxNode {
        self.tree.syntax()
    }

    pub fn green_node(&self) -> rowan::GreenNode {
        self.syntax().green().into_owned()
    }
}

fn parse_impl(input: &str) -> Parsed {
    let tokens = lex(input);
    let mut parser = Parser::new(tokens.clone());
    parser.parse_file();
    let diagnostics = parser.diagnostics.clone();
    let syntax = parser.finish();
    Parsed {
        diagnostics,
        syntax,
    }
}

struct Parsed {
    diagnostics: Vec<SyntaxDiagnostic>,
    syntax: SyntaxNode,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<SyntaxDiagnostic>,
    builder: rowan::GreenNodeBuilder<'static>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            builder: rowan::GreenNodeBuilder::new(),
        }
    }

    fn finish(mut self) -> SyntaxNode {
        while !self.at(TokenKind::Eof) {
            self.bump();
        }
        self.bump_trivia();
        let green = self.builder.finish();
        SyntaxNode::new_root(green)
    }

    fn parse_file(&mut self) {
        let m = self.start_node(SyntaxKind::Root);
        self.bump_trivia();
        while self.at_keyword("set") || self.at_keyword("param") {
            if self.at_keyword("set") {
                self.parse_directive();
            } else {
                self.parse_param_declaration();
            }
            self.bump_trivia();
        }

        if !self.at(TokenKind::Eof) {
            self.parse_query();
        }

        while !self.at(TokenKind::Eof) {
            if self.eat(TokenKind::Semicolon).is_some() {
                continue;
            }
            let token = self.bump();
            self.error_at(
                format!("unexpected token `{}` after query", token.text),
                token.range,
            );
        }

        self.bump_trivia();
        self.finish_node(m);
    }

    fn parse_directive(&mut self) -> TextRange {
        let m = self.start_node(SyntaxKind::Directive);
        let start = self.expect_keyword("set").range.start;
        let _name = self.parse_name().map(|it| it.text).unwrap_or_else(|| {
            self.error_here("expected directive name");
            String::new()
        });
        let value = self.eat(TokenKind::Eq).map(|_| self.parse_expr());
        let end = if let Some(semi) = self.eat(TokenKind::Semicolon) {
            semi.range.end
        } else {
            self.error_here("expected `;` after directive");
            value.unwrap_or(TextRange::empty(start)).end
        };

        self.finish_node(m);
        TextRange::new(start, end)
    }

    fn parse_param_declaration(&mut self) -> TextRange {
        let start = self.expect_keyword("param").range.start;

        if self.at(TokenKind::Param) {
            self.bump();
        } else {
            self.error_here("expected parameter name");
        }

        if self.eat(TokenKind::Colon).is_none() {
            self.error_here("expected `:` in parameter declaration");
        }

        while !self.at(TokenKind::Semicolon) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Pipe) {
                self.error_here("expected `;` after parameter declaration");
                break;
            }
            self.bump();
        }

        let end = self
            .eat(TokenKind::Semicolon)
            .map(|it| it.range.end)
            .unwrap_or_else(|| self.current().range.start);

        TextRange::new(start, end)
    }

    fn parse_query(&mut self) -> TextRange {
        if self.at(TokenKind::LParen) {
            self.parse_compute_query()
        } else {
            self.parse_simple_query()
        }
    }

    fn parse_compute_query(&mut self) -> TextRange {
        let m = self.start_node(SyntaxKind::ComputeQuery);
        let start = self.expect(TokenKind::LParen, "expected `(`").range.start;

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Comma) {
                let comma = self.bump();
                self.error_at("expected query before `,`", comma.range);
                continue;
            }

            self.parse_query();

            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            if !self.at(TokenKind::RParen) {
                self.error_here("expected `,` or `)` after compute input");
                self.recover_until(&[TokenKind::Comma, TokenKind::RParen, TokenKind::Eof]);
                let _ = self.eat(TokenKind::Comma);
            }
        }

        let mut end = self
            .eat(TokenKind::RParen)
            .map(|it| it.range.end)
            .unwrap_or_else(|| {
                self.error_here("expected `)` after compute inputs");
                self.current().range.end
            });

        if self.at(TokenKind::Pipe) && self.nth_keyword(1, "compute") {
            let rule = self.parse_compute_rule();
            end = rule.end;
        } else {
            self.error_here("expected `| compute` after compute inputs");
        }

        while self.at(TokenKind::Pipe) {
            if self.nth_keyword(1, "compute") {
                let rule = self.parse_compute_rule();
                self.error_at("duplicate compute rule", rule);
                end = rule.end;
            } else {
                let pipe = self.parse_pipe();
                end = pipe.end;
            }
        }

        self.finish_node(m);
        TextRange::new(start, end)
    }

    fn parse_compute_rule(&mut self) -> TextRange {
        let m = self.start_node(SyntaxKind::ComputeRule);
        let start = self.expect(TokenKind::Pipe, "expected `|`").range.start;
        self.expect_keyword("compute");
        let name = if self.can_start_name() && !self.nth_keyword(0, "using") {
            self.parse_name()
        } else {
            None
        };

        if !self.at_keyword("using") {
            self.error_here("expected `using` in compute rule");
        } else {
            self.bump();
        }

        let function = if self.can_start_function_name() {
            Some(self.parse_function_call())
        } else {
            self.error_here("expected compute function");
            None
        };
        let end = function
            .as_ref()
            .map(|it| it.range.end)
            .or_else(|| name.as_ref().map(|it| it.range.end))
            .unwrap_or(self.current().range.end);

        self.finish_node(m);
        TextRange::new(start, end)
    }

    fn parse_simple_query(&mut self) -> TextRange {
        let m = self.start_node(SyntaxKind::SimpleQuery);
        let start = self.current().range.start;
        let source = if self.can_start_name() {
            Some(self.parse_source())
        } else {
            None
        };

        let mut pipes = Vec::new();
        while self.at(TokenKind::Pipe) {
            if self.nth_keyword(1, "compute") {
                break;
            }
            pipes.push(self.parse_pipe());
        }

        let end = pipes
            .last()
            .map(|it| it.end)
            .or_else(|| source.as_ref().map(|it| it.end))
            .unwrap_or(start);

        self.finish_node(m);
        TextRange::new(start, end)
    }

    fn parse_source(&mut self) -> TextRange {
        let m = self.start_node(SyntaxKind::Source);
        let start = self.current().range.start;
        let dataset = self.parse_name();
        let metric = if self.eat(TokenKind::Colon).is_some() {
            self.parse_name().or_else(|| {
                self.error_here("expected metric name after `:`");
                None
            })
        } else {
            self.error_here("expected `:` between dataset and metric");
            None
        };

        let time_range = if self.at(TokenKind::LBracket) {
            Some(self.parse_time_range())
        } else {
            None
        };

        let alias = if self.at_keyword("as") {
            self.bump();
            self.parse_name().or_else(|| {
                self.error_here("expected alias after `as`");
                None
            })
        } else {
            None
        };

        let end = alias
            .as_ref()
            .map(|it| it.range.end)
            .or_else(|| time_range.as_ref().map(|it| it.end))
            .or_else(|| metric.as_ref().map(|it| it.range.end))
            .or_else(|| dataset.as_ref().map(|it| it.range.end))
            .unwrap_or(start);

        self.finish_node(m);
        TextRange::new(start, end)
    }

    fn parse_time_range(&mut self) -> TextRange {
        let m = self.start_node(SyntaxKind::TimeRange);
        let start = self.expect(TokenKind::LBracket, "expected `[`").range.start;
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Pipe) || self.at(TokenKind::Semicolon) {
                self.error_here("expected `]` after range");
                break;
            }
            self.bump();
        }
        let end = self
            .eat(TokenKind::RBracket)
            .map(|it| it.range.end)
            .unwrap_or_else(|| self.current().range.start);
        self.finish_node(m);
        TextRange::new(start, end)
    }

    fn parse_pipe(&mut self) -> TextRange {
        let m = self.start_node(self.pipe_node_kind());
        let pipe = self.expect(TokenKind::Pipe, "expected `|`");
        let range = if self.at_keyword("where") || self.at_keyword("filter") {
            let keyword = self.bump();
            let expr = if self.is_pipe_end() {
                self.error_here("expected filter expression");
                None
            } else {
                Some(self.parse_filter_expr())
            };
            let end = expr.as_ref().map(|it| it.end).unwrap_or(keyword.range.end);
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("map") {
            let keyword = self.bump();
            let end = if self.can_start_operator() {
                self.bump();
                let value = self.parse_expr();
                value.end
            } else if self.can_start_function_name() {
                self.parse_function_call().range.end
            } else {
                self.error_here("expected map function");
                keyword.range.end
            };
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("align") {
            let keyword = self.bump();
            let window = if self.at_keyword("to") {
                self.bump();
                if self.can_start_literal_or_param() {
                    Some(self.parse_expr())
                } else {
                    self.error_here("expected align window after `to`");
                    None
                }
            } else if self.can_start_literal_or_param() {
                // Keep the older shorthand as recoverable input for editor use.
                Some(self.parse_expr())
            } else {
                None
            };
            let over = if self.at_keyword("over") {
                self.bump();
                if self.can_start_literal_or_param() {
                    Some(self.parse_expr())
                } else {
                    self.error_here("expected align window after `over`");
                    None
                }
            } else {
                None
            };
            let function = self.parse_optional_using_function();
            if window.is_none() && function.is_none() {
                self.error_here("expected align window or `using` function");
            }
            let end = function
                .as_ref()
                .map(|it| it.end)
                .or_else(|| over.as_ref().map(|it| it.end))
                .or_else(|| window.as_ref().map(|it| it.end))
                .unwrap_or(keyword.range.end);
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("group") {
            let keyword = self.bump();
            let tags = self.parse_optional_tags();
            let function = self.parse_optional_using_function();
            let end = function
                .as_ref()
                .map(|it| it.end)
                .or(tags)
                .unwrap_or(keyword.range.end);
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("bucket") {
            let keyword = self.bump();
            let tags = self.parse_optional_tags();
            let window = if self.at_keyword("to") {
                self.bump();
                Some(self.parse_expr())
            } else if tags.is_none() && self.can_start_literal_or_param() {
                Some(self.parse_expr())
            } else {
                None
            };
            let function = self.parse_optional_using_function();
            let end = function
                .as_ref()
                .map(|it| it.end)
                .or_else(|| window.as_ref().map(|it| it.end))
                .or(tags)
                .unwrap_or(keyword.range.end);
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("extend") {
            let keyword = self.bump();
            let assigns = self.parse_assignments();
            let end = assigns.unwrap_or(keyword.range.end);
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("as") {
            let keyword = self.bump();
            let alias = self.parse_name().or_else(|| {
                self.error_here("expected alias after `as`");
                None
            });
            let end = alias
                .as_ref()
                .map(|it| it.range.end)
                .unwrap_or(keyword.range.end);
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("ifdef") {
            let end = self.parse_ifdef_pipe_tail();
            TextRange::new(pipe.range.start, end)
        } else if self.at_keyword("sample") {
            let keyword = self.bump();
            if !self.is_pipe_end() {
                self.parse_expr();
            }
            TextRange::new(
                pipe.range.start,
                self.previous_end().unwrap_or(keyword.range.end),
            )
        } else {
            if self.can_start_name() {
                self.parse_name();
            }
            self.recover_until(&[
                TokenKind::Pipe,
                TokenKind::Semicolon,
                TokenKind::RParen,
                TokenKind::Eof,
            ]);
            TextRange::new(pipe.range.start, self.current().range.start)
        };
        self.finish_node(m);
        range
    }

    fn parse_optional_tags(&mut self) -> Option<usize> {
        if self.at_keyword("by") {
            self.bump();
        }

        let mut end = None;
        while self.can_start_name() && !self.at_keyword("using") && !self.at_keyword("to") {
            end = self.parse_name().map(|it| it.range.end);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        end
    }

    fn parse_optional_using_function(&mut self) -> Option<TextRange> {
        if self.at_keyword("using") {
            self.bump();
            if self.can_start_function_name() {
                Some(self.parse_function_call().range)
            } else {
                self.error_here("expected function after `using`");
                None
            }
        } else {
            None
        }
    }

    fn parse_ifdef_pipe_tail(&mut self) -> usize {
        let keyword = self.expect_keyword("ifdef");
        if self.at(TokenKind::LParen) {
            self.bump_balanced(TokenKind::LParen, TokenKind::RParen);
        }

        if self.at(TokenKind::LBrace) {
            self.bump_balanced(TokenKind::LBrace, TokenKind::RBrace);
        }

        if self.at_keyword("else") {
            self.bump();
            if self.at(TokenKind::LBrace) {
                self.bump_balanced(TokenKind::LBrace, TokenKind::RBrace);
            }
        }

        self.previous_end().unwrap_or(keyword.range.end)
    }

    fn bump_balanced(&mut self, open: TokenKind, close: TokenKind) {
        if !self.at(open) {
            return;
        }

        let mut depth = 0usize;
        while !self.at(TokenKind::Eof) {
            if self.at(open) {
                depth += 1;
            } else if self.at(close) {
                depth = depth.saturating_sub(1);
                self.bump();
                if depth == 0 {
                    return;
                }
                continue;
            }
            self.bump();
        }
    }

    fn parse_assignments(&mut self) -> Option<usize> {
        let mut last_end = None;
        while !self.is_pipe_end() {
            let m = self.start_node(SyntaxKind::Assignment);
            let start = self.current().range.start;
            let name = self.parse_name().or_else(|| {
                self.error_here("expected assignment name");
                None
            });
            let value = if self.eat(TokenKind::Eq).is_some() {
                Some(self.parse_expr())
            } else {
                self.error_here("expected `=` in assignment");
                None
            };
            let end = value
                .as_ref()
                .map(|it| it.end)
                .or_else(|| name.as_ref().map(|it| it.range.end))
                .unwrap_or(start);
            last_end = Some(end);
            self.finish_node(m);

            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        last_end
    }

    fn parse_filter_expr(&mut self) -> TextRange {
        self.parse_filter_or()
    }

    fn parse_filter_or(&mut self) -> TextRange {
        let mut lhs = self.parse_filter_and();
        while self.at_keyword("or") {
            self.bump();
            let rhs = self.parse_filter_and();
            lhs = TextRange::new(lhs.start, rhs.end);
        }
        lhs
    }

    fn parse_filter_and(&mut self) -> TextRange {
        let mut lhs = self.parse_filter_not();
        while self.at_keyword("and") {
            self.bump();
            let rhs = self.parse_filter_not();
            lhs = TextRange::new(lhs.start, rhs.end);
        }
        lhs
    }

    fn parse_filter_not(&mut self) -> TextRange {
        if self.at_keyword("not") {
            let m = self.start_node(SyntaxKind::NotExpr);
            let op = self.bump();
            let expr = self.parse_filter_not();
            let expr = TextRange::new(op.range.start, expr.end);
            self.finish_node(m);
            expr
        } else {
            self.parse_filter_primary()
        }
    }

    fn parse_filter_primary(&mut self) -> TextRange {
        if self.at(TokenKind::LParen) {
            let m = self.start_node(SyntaxKind::ParenExpr);
            let start = self.bump().range.start;
            let expr = self.parse_filter_expr();
            let end = self
                .eat(TokenKind::RParen)
                .map(|it| it.range.end)
                .unwrap_or_else(|| {
                    self.error_here("expected `)` after filter expression");
                    expr.end
                });
            self.finish_node(m);
            return TextRange::new(start, end);
        }

        let checkpoint = self.checkpoint();
        let lhs = match self.parse_name() {
            Some(name) => name,
            None => {
                let m = self.start_node(SyntaxKind::MissingExpr);
                let range = self.current().range;
                self.error_here("expected filter field");
                if !self.is_pipe_end() {
                    self.bump();
                }
                self.finish_node(m);
                return range;
            }
        };

        if self.at_keyword("is") {
            self.start_node_at(checkpoint, SyntaxKind::TypeCheckExpr);
            self.bump();
            let ty = self.parse_name().unwrap_or_else(|| {
                self.error_here("expected type name after `is`");
                ParsedName {
                    text: String::new(),
                    range: TextRange::empty(self.current().range.start),
                }
            });
            let expr = TextRange::new(lhs.range.start, ty.range.end);
            self.finish_node(Marker);
            return expr;
        }

        let _op = if self.at(TokenKind::Cmp) || self.at(TokenKind::Eq) {
            self.start_node_at(checkpoint, SyntaxKind::CompareExpr);
            self.bump()
        } else {
            self.start_node_at(checkpoint, SyntaxKind::MissingExpr);
            self.error_here("expected comparison operator");
            let end = if !self.is_pipe_end() {
                self.parse_expr().end
            } else {
                lhs.range.end
            };
            let expr = TextRange::new(lhs.range.start, end);
            self.finish_node(Marker);
            return expr;
        };
        let rhs = self.parse_expr();
        let expr = TextRange::new(lhs.range.start, rhs.end);
        self.finish_node(Marker);
        expr
    }

    fn parse_expr(&mut self) -> TextRange {
        match self.current().kind {
            TokenKind::String => {
                let m = self.start_node(SyntaxKind::StringExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            TokenKind::Number => {
                let m = self.start_node(SyntaxKind::NumberExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            TokenKind::Duration => {
                let m = self.start_node(SyntaxKind::DurationExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            TokenKind::Timestamp => {
                let m = self.start_node(SyntaxKind::TimestampExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            TokenKind::Bool => {
                let m = self.start_node(SyntaxKind::BoolExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            TokenKind::Regex => {
                let m = self.start_node(SyntaxKind::RegexExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            TokenKind::Param => {
                let m = self.start_node(SyntaxKind::ParamExpr);
                let token = self.bump();
                self.finish_node(m);
                token.range
            }
            _ if self.can_start_function_name() => {
                let m = self.start_node(SyntaxKind::CallExpr);
                let call_or_name = self.parse_function_call();
                let range = call_or_name.range;
                self.finish_node(m);
                range
            }
            _ => {
                let m = self.start_node(SyntaxKind::MissingExpr);
                let range = self.current().range;
                self.error_here("expected expression");
                if !self.is_expr_end() {
                    self.bump();
                }
                self.finish_node(m);
                range
            }
        }
    }

    fn parse_function_call(&mut self) -> ParsedCall {
        let m = self.start_node(SyntaxKind::FunctionCall);
        let name = if self.can_start_operator() {
            let token = self.bump();
            ParsedName {
                text: token.text,
                range: token.range,
            }
        } else {
            self.parse_name().unwrap_or_else(|| {
                self.error_here("expected function name");
                ParsedName {
                    text: String::new(),
                    range: TextRange::empty(self.current().range.start),
                }
            })
        };

        let mut end = name.range.end;
        if self.eat(TokenKind::LParen).is_some() {
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                if self.at(TokenKind::Comma) {
                    let comma = self.bump();
                    self.error_at("expected argument before `,`", comma.range);
                    continue;
                }
                self.parse_expr();
                if self.eat(TokenKind::Comma).is_some() {
                    continue;
                }
                if !self.at(TokenKind::RParen) {
                    self.error_here("expected `,` or `)` after argument");
                    self.recover_until(&[
                        TokenKind::Comma,
                        TokenKind::RParen,
                        TokenKind::Pipe,
                        TokenKind::Eof,
                    ]);
                    let _ = self.eat(TokenKind::Comma);
                }
            }
            end = self
                .eat(TokenKind::RParen)
                .map(|it| it.range.end)
                .unwrap_or_else(|| {
                    self.error_here("expected `)` after arguments");
                    end
                });
        }

        let call = ParsedCall {
            range: TextRange::new(name.range.start, end),
        };
        self.finish_node(m);
        call
    }

    fn parse_name(&mut self) -> Option<ParsedName> {
        if !self.can_start_name() {
            return None;
        }

        let m = self.start_node(SyntaxKind::NameRef);
        let first = self.bump();
        let start = first.range.start;
        let mut end = first.range.end;
        let mut text = first.text;

        while (self.at(TokenKind::DoubleColon) || self.at(TokenKind::Dot))
            && self.nth_can_start_name(1)
        {
            let sep = self.bump();
            let segment = self.bump();
            text.push_str(&sep.text);
            text.push_str(&segment.text);
            end = segment.range.end;
        }

        let name = ParsedName {
            text,
            range: TextRange::new(start, end),
        };
        self.finish_node(m);
        Some(name)
    }

    fn recover_until(&mut self, kinds: &[TokenKind]) {
        while !kinds.iter().any(|kind| self.at(*kind)) {
            self.bump();
        }
    }

    fn is_pipe_end(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Pipe
                | TokenKind::Comma
                | TokenKind::Semicolon
                | TokenKind::RParen
                | TokenKind::Eof
        )
    }

    fn previous_end(&self) -> Option<usize> {
        self.pos
            .checked_sub(1)
            .and_then(|idx| self.tokens.get(idx))
            .map(|token| token.range.end)
    }

    fn is_expr_end(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Pipe
                | TokenKind::Comma
                | TokenKind::Semicolon
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::Eof
        )
    }

    fn can_start_literal_or_param(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::String
                | TokenKind::Number
                | TokenKind::Duration
                | TokenKind::Timestamp
                | TokenKind::Bool
                | TokenKind::Regex
                | TokenKind::Param
        )
    }

    fn can_start_function_name(&self) -> bool {
        self.can_start_name() || self.can_start_operator()
    }

    fn can_start_name(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Ident | TokenKind::EscapedIdent | TokenKind::Keyword | TokenKind::Param
        )
    }

    fn nth_can_start_name(&self, n: usize) -> bool {
        matches!(
            self.nth(n).kind,
            TokenKind::Ident | TokenKind::EscapedIdent | TokenKind::Keyword | TokenKind::Param
        )
    }

    fn can_start_operator(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash
        )
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn at_keyword(&self, text: &str) -> bool {
        self.current().kind == TokenKind::Keyword && self.current().text == text
    }

    fn nth_keyword(&self, n: usize, text: &str) -> bool {
        let token = self.nth(n);
        token.kind == TokenKind::Keyword && token.text == text
    }

    fn pipe_node_kind(&self) -> SyntaxKind {
        if self.nth_keyword(1, "where") || self.nth_keyword(1, "filter") {
            SyntaxKind::WherePipe
        } else if self.nth_keyword(1, "map") {
            SyntaxKind::MapPipe
        } else if self.nth_keyword(1, "align") {
            SyntaxKind::AlignPipe
        } else if self.nth_keyword(1, "group") {
            SyntaxKind::GroupPipe
        } else if self.nth_keyword(1, "bucket") {
            SyntaxKind::BucketPipe
        } else if self.nth_keyword(1, "extend") {
            SyntaxKind::ExtendPipe
        } else if self.nth_keyword(1, "as") {
            SyntaxKind::AsPipe
        } else if self.nth_keyword(1, "ifdef") || self.nth_keyword(1, "sample") {
            SyntaxKind::MapPipe
        } else {
            SyntaxKind::UnknownPipe
        }
    }

    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Token {
        if self.at(kind) {
            self.bump()
        } else {
            self.error_here(message);
            Token {
                kind,
                text: String::new(),
                range: TextRange::empty(self.current().range.start),
            }
        }
    }

    fn expect_keyword(&mut self, text: &str) -> Token {
        if self.at_keyword(text) {
            self.bump()
        } else {
            self.error_here(format!("expected `{text}`"));
            Token {
                kind: TokenKind::Keyword,
                text: text.to_string(),
                range: TextRange::empty(self.current().range.start),
            }
        }
    }

    fn start_node(&mut self, kind: SyntaxKind) -> Marker {
        if self.pos > 0 {
            self.bump_trivia();
        }
        self.builder.start_node(kind.into());
        Marker
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.builder.checkpoint()
    }

    fn start_node_at(&mut self, checkpoint: rowan::Checkpoint, kind: SyntaxKind) -> Marker {
        self.builder.start_node_at(checkpoint, kind.into());
        Marker
    }

    fn finish_node(&mut self, _marker: Marker) {
        self.builder.finish_node();
    }

    fn bump(&mut self) -> Token {
        self.bump_trivia();
        let token = self.tokens[self.pos].clone();
        if token.kind != TokenKind::Eof {
            self.builder
                .token(SyntaxKind::from(token.kind).into(), &token.text);
            self.pos += 1;
        }
        token
    }

    fn current(&self) -> &Token {
        self.nth(0)
    }

    fn nth(&self, n: usize) -> &Token {
        let mut idx = self.pos;
        let mut remaining = n;
        while idx < self.tokens.len() {
            match self.tokens[idx].kind {
                TokenKind::Whitespace | TokenKind::Comment => idx += 1,
                _ if remaining == 0 => return &self.tokens[idx],
                _ => {
                    remaining -= 1;
                    idx += 1;
                }
            }
        }
        self.tokens.last().expect("lexer always appends EOF")
    }

    fn bump_trivia(&mut self) {
        while matches!(
            self.tokens[self.pos].kind,
            TokenKind::Whitespace | TokenKind::Comment
        ) {
            let token = &self.tokens[self.pos];
            self.builder
                .token(SyntaxKind::from(token.kind).into(), &token.text);
            self.pos += 1;
        }
    }

    fn error_here(&mut self, message: impl Into<String>) {
        self.error_at(message, self.current().range);
    }

    fn error_at(&mut self, message: impl Into<String>, range: TextRange) {
        self.diagnostics.push(SyntaxDiagnostic {
            message: message.into(),
            range,
        });
    }
}

#[derive(Debug, Clone, Copy)]
struct Marker;

#[derive(Debug, Clone)]
struct ParsedName {
    text: String,
    range: TextRange,
}

#[derive(Debug, Clone)]
struct ParsedCall {
    range: TextRange,
}
