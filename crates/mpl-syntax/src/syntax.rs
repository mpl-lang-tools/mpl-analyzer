//! Rowan language integration and typed AST wrappers.
//!
//! This module defines MPL syntax kinds, the rowan language marker, CST aliases,
//! and thin typed wrappers over syntax nodes. Wrappers provide ergonomic
//! traversal while preserving the underlying lossless tree.

use rowan::Language;

use crate::lexer::TokenKind;
use crate::text::{TextRange, TextSize as MplTextSize};

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
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

    Root,
    Directive,
    SimpleQuery,
    ComputeQuery,
    Source,
    TimeRange,
    WherePipe,
    MapPipe,
    AlignPipe,
    GroupPipe,
    BucketPipe,
    ExtendPipe,
    AsPipe,
    UnknownPipe,
    ComputeRule,
    Assignment,
    NameRef,
    FunctionCall,
    StringExpr,
    NumberExpr,
    DurationExpr,
    TimestampExpr,
    BoolExpr,
    RegexExpr,
    ParamExpr,
    NameExpr,
    CallExpr,
    MissingExpr,
    BinaryExpr,
    NotExpr,
    CompareExpr,
    TypeCheckExpr,
    ParenExpr,

    __Last,
}

impl From<TokenKind> for SyntaxKind {
    fn from(kind: TokenKind) -> Self {
        match kind {
            TokenKind::Whitespace => SyntaxKind::Whitespace,
            TokenKind::Comment => SyntaxKind::Comment,
            TokenKind::Ident => SyntaxKind::Ident,
            TokenKind::EscapedIdent => SyntaxKind::EscapedIdent,
            TokenKind::Param => SyntaxKind::Param,
            TokenKind::String => SyntaxKind::String,
            TokenKind::Regex => SyntaxKind::Regex,
            TokenKind::Number => SyntaxKind::Number,
            TokenKind::Duration => SyntaxKind::Duration,
            TokenKind::Timestamp => SyntaxKind::Timestamp,
            TokenKind::Bool => SyntaxKind::Bool,
            TokenKind::Pipe => SyntaxKind::Pipe,
            TokenKind::Colon => SyntaxKind::Colon,
            TokenKind::DoubleColon => SyntaxKind::DoubleColon,
            TokenKind::Comma => SyntaxKind::Comma,
            TokenKind::Semicolon => SyntaxKind::Semicolon,
            TokenKind::Dot => SyntaxKind::Dot,
            TokenKind::DotDot => SyntaxKind::DotDot,
            TokenKind::LParen => SyntaxKind::LParen,
            TokenKind::RParen => SyntaxKind::RParen,
            TokenKind::LBrace => SyntaxKind::LBrace,
            TokenKind::RBrace => SyntaxKind::RBrace,
            TokenKind::LBracket => SyntaxKind::LBracket,
            TokenKind::RBracket => SyntaxKind::RBracket,
            TokenKind::Eq => SyntaxKind::Eq,
            TokenKind::Cmp => SyntaxKind::Cmp,
            TokenKind::Plus => SyntaxKind::Plus,
            TokenKind::Minus => SyntaxKind::Minus,
            TokenKind::Star => SyntaxKind::Star,
            TokenKind::Slash => SyntaxKind::Slash,
            TokenKind::Keyword => SyntaxKind::Keyword,
            TokenKind::Unknown => SyntaxKind::Unknown,
            TokenKind::Eof => SyntaxKind::Eof,
        }
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        rowan::SyntaxKind(kind as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MplLanguage {}

impl Language for MplLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        assert!(raw.0 < SyntaxKind::__Last as u16);
        // SAFETY: SyntaxKind is repr(u16), and the assertion keeps raw values in range.
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<MplLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<MplLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<MplLanguage>;

pub trait AstNode: Sized {
    fn can_cast(kind: SyntaxKind) -> bool;

    fn cast(syntax: SyntaxNode) -> Option<Self>;

    fn syntax(&self) -> &SyntaxNode;

    fn range(&self) -> TextRange {
        text_range(self.syntax().text_range())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PipeKind {
    Where,
    Map,
    Align,
    Group,
    Bucket,
    Extend,
    As,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExprKind {
    String,
    Number,
    Duration,
    Timestamp,
    Bool,
    Regex,
    Param,
    Name,
    Call,
    Missing,
    Binary,
    Not,
    Compare,
    TypeCheck,
    Paren,
}

macro_rules! ast_node {
    ($name:ident, $kind:pat) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            syntax: SyntaxNode,
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, $kind)
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                Self::can_cast(syntax.kind()).then_some(Self { syntax })
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

ast_node!(SourceFileNode, SyntaxKind::Root);
ast_node!(DirectiveNode, SyntaxKind::Directive);
ast_node!(
    QueryNode,
    SyntaxKind::SimpleQuery | SyntaxKind::ComputeQuery
);
ast_node!(SourceNode, SyntaxKind::Source);
ast_node!(ComputeRuleNode, SyntaxKind::ComputeRule);
ast_node!(
    PipeNode,
    SyntaxKind::WherePipe
        | SyntaxKind::MapPipe
        | SyntaxKind::AlignPipe
        | SyntaxKind::GroupPipe
        | SyntaxKind::BucketPipe
        | SyntaxKind::ExtendPipe
        | SyntaxKind::AsPipe
        | SyntaxKind::UnknownPipe
);
ast_node!(TimeRangeNode, SyntaxKind::TimeRange);
ast_node!(NameRefNode, SyntaxKind::NameRef);
ast_node!(FunctionCallNode, SyntaxKind::FunctionCall);
ast_node!(AssignmentNode, SyntaxKind::Assignment);
ast_node!(
    ExprNode,
    SyntaxKind::StringExpr
        | SyntaxKind::NumberExpr
        | SyntaxKind::DurationExpr
        | SyntaxKind::TimestampExpr
        | SyntaxKind::BoolExpr
        | SyntaxKind::RegexExpr
        | SyntaxKind::ParamExpr
        | SyntaxKind::NameExpr
        | SyntaxKind::CallExpr
        | SyntaxKind::MissingExpr
        | SyntaxKind::BinaryExpr
        | SyntaxKind::NotExpr
        | SyntaxKind::CompareExpr
        | SyntaxKind::TypeCheckExpr
        | SyntaxKind::ParenExpr
);

impl SourceFileNode {
    pub fn directives(&self) -> impl Iterator<Item = DirectiveNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn queries(&self) -> impl Iterator<Item = QueryNode> + '_ {
        ast_children(self.syntax())
    }
}

impl QueryNode {
    pub fn input_queries(&self) -> impl Iterator<Item = QueryNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn sources(&self) -> impl Iterator<Item = SourceNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn pipes(&self) -> impl Iterator<Item = PipeNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn compute_rule(&self) -> Option<ComputeRuleNode> {
        ast_children(self.syntax()).next()
    }
}

impl DirectiveNode {
    pub fn name(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).next()
    }

    pub fn value(&self) -> Option<ExprNode> {
        ast_children(self.syntax()).next()
    }
}

impl SourceNode {
    pub fn dataset(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).next()
    }

    pub fn metric(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).nth(1)
    }

    pub fn time_range(&self) -> Option<TimeRangeNode> {
        ast_children(self.syntax()).next()
    }

    pub fn alias(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).nth(2)
    }
}

impl TimeRangeNode {
    pub fn text(&self) -> String {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| {
                !matches!(
                    token.kind(),
                    SyntaxKind::Whitespace
                        | SyntaxKind::Comment
                        | SyntaxKind::LBracket
                        | SyntaxKind::RBracket
                )
            })
            .map(|token| token.text().to_owned())
            .collect::<String>()
            .trim()
            .to_owned()
    }
}

impl PipeNode {
    pub fn kind(&self) -> PipeKind {
        match self.syntax.kind() {
            SyntaxKind::WherePipe => PipeKind::Where,
            SyntaxKind::MapPipe => PipeKind::Map,
            SyntaxKind::AlignPipe => PipeKind::Align,
            SyntaxKind::GroupPipe => PipeKind::Group,
            SyntaxKind::BucketPipe => PipeKind::Bucket,
            SyntaxKind::ExtendPipe => PipeKind::Extend,
            SyntaxKind::AsPipe => PipeKind::As,
            SyntaxKind::UnknownPipe => PipeKind::Unknown,
            _ => unreachable!("PipeNode only casts pipe syntax nodes"),
        }
    }

    pub fn keyword(&self) -> Option<SyntaxToken> {
        let mut tokens = significant_tokens(self.syntax()).into_iter();
        tokens.find(|token| token.kind() == SyntaxKind::Pipe)?;
        tokens.next()
    }

    pub fn keyword_text(&self) -> Option<String> {
        self.keyword().map(|token| token.text().to_owned())
    }

    pub fn function(&self) -> Option<FunctionCallNode> {
        ast_children(self.syntax()).next()
    }

    pub fn expr(&self) -> Option<ExprNode> {
        self.exprs().next()
    }

    pub fn exprs(&self) -> impl Iterator<Item = ExprNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn expr_descendants(&self) -> impl Iterator<Item = ExprNode> + '_ {
        self.syntax().descendants().filter_map(ExprNode::cast)
    }

    pub fn assignments(&self) -> impl Iterator<Item = AssignmentNode> + '_ {
        ast_children(self.syntax())
    }
}

impl ComputeRuleNode {
    pub fn name(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).next()
    }

    pub fn function(&self) -> Option<FunctionCallNode> {
        ast_children(self.syntax()).next()
    }
}

impl NameRefNode {
    pub fn text(&self) -> String {
        self.syntax.text().to_string()
    }

    pub fn tokens(&self) -> impl Iterator<Item = SyntaxToken> + '_ {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !is_trivia(token.kind()))
    }

    pub fn token_texts(&self) -> impl Iterator<Item = String> + '_ {
        self.tokens().map(|token| token.text().to_owned())
    }
}

impl FunctionCallNode {
    pub fn name(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).next()
    }

    pub fn callee(&self) -> Option<SyntaxToken> {
        first_significant_token(self.syntax())
    }

    pub fn callee_text(&self) -> Option<String> {
        self.name()
            .map(|name| name.text())
            .or_else(|| self.operator().map(|token| token.text().to_owned()))
    }

    pub fn operator(&self) -> Option<SyntaxToken> {
        self.callee()
            .filter(|token| is_operator_token(token.kind()))
    }

    pub fn is_operator(&self) -> bool {
        self.operator().is_some()
    }

    pub fn args(&self) -> impl Iterator<Item = ExprNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn has_arg_list(&self) -> bool {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .any(|token| token.kind() == SyntaxKind::LParen)
    }
}

impl AssignmentNode {
    pub fn name(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).next()
    }

    pub fn value(&self) -> Option<ExprNode> {
        ast_children(self.syntax()).next()
    }
}

impl ExprNode {
    pub fn kind(&self) -> ExprKind {
        match self.syntax.kind() {
            SyntaxKind::StringExpr => ExprKind::String,
            SyntaxKind::NumberExpr => ExprKind::Number,
            SyntaxKind::DurationExpr => ExprKind::Duration,
            SyntaxKind::TimestampExpr => ExprKind::Timestamp,
            SyntaxKind::BoolExpr => ExprKind::Bool,
            SyntaxKind::RegexExpr => ExprKind::Regex,
            SyntaxKind::ParamExpr => ExprKind::Param,
            SyntaxKind::NameExpr => ExprKind::Name,
            SyntaxKind::CallExpr => ExprKind::Call,
            SyntaxKind::MissingExpr => ExprKind::Missing,
            SyntaxKind::BinaryExpr => ExprKind::Binary,
            SyntaxKind::NotExpr => ExprKind::Not,
            SyntaxKind::CompareExpr => ExprKind::Compare,
            SyntaxKind::TypeCheckExpr => ExprKind::TypeCheck,
            SyntaxKind::ParenExpr => ExprKind::Paren,
            _ => unreachable!("ExprNode only casts expression syntax nodes"),
        }
    }

    pub fn text(&self) -> String {
        self.syntax.text().to_string()
    }

    pub fn token(&self) -> Option<SyntaxToken> {
        first_significant_token(self.syntax())
    }

    pub fn token_text(&self) -> Option<String> {
        self.token().map(|token| token.text().to_owned())
    }

    pub fn name(&self) -> Option<NameRefNode> {
        ast_children(self.syntax()).next()
    }

    pub fn function_call(&self) -> Option<FunctionCallNode> {
        ast_children(self.syntax()).next()
    }

    pub fn exprs(&self) -> impl Iterator<Item = ExprNode> + '_ {
        ast_children(self.syntax())
    }

    pub fn expr_descendants(&self) -> impl Iterator<Item = ExprNode> + '_ {
        self.syntax().descendants().filter_map(ExprNode::cast)
    }
}

pub fn ast_children<'a, N: AstNode + 'a>(node: &'a SyntaxNode) -> impl Iterator<Item = N> + 'a {
    node.children().filter_map(N::cast)
}

pub fn text_size(size: rowan::TextSize) -> MplTextSize {
    u32::from(size) as MplTextSize
}

pub fn text_range(range: rowan::TextRange) -> TextRange {
    TextRange::new(text_size(range.start()), text_size(range.end()))
}

pub fn token_range(token: &SyntaxToken) -> TextRange {
    text_range(token.text_range())
}

pub fn token_text(token: &SyntaxToken) -> &str {
    token.text()
}

pub fn debug_tree(node: &SyntaxNode) -> String {
    let mut out = String::new();
    fmt_node(&mut out, node, 0);
    out
}

fn fmt_node(out: &mut String, node: &SyntaxNode, indent: usize) {
    out.push_str(&"  ".repeat(indent));
    out.push_str(&format!("{:?}\n", node.kind()));
    for child in node.children_with_tokens() {
        match child {
            rowan::NodeOrToken::Node(node) => fmt_node(out, &node, indent + 1),
            rowan::NodeOrToken::Token(token) => {
                out.push_str(&"  ".repeat(indent + 1));
                out.push_str(&format!("{:?} {:?}\n", token.kind(), token.text()));
            }
        }
    }
}

fn significant_tokens(node: &SyntaxNode) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    collect_significant_tokens(node, &mut tokens);
    tokens
}

fn collect_significant_tokens(node: &SyntaxNode, tokens: &mut Vec<SyntaxToken>) {
    for element in node.children_with_tokens() {
        match element {
            rowan::NodeOrToken::Node(node) => collect_significant_tokens(&node, tokens),
            rowan::NodeOrToken::Token(token) if !is_trivia(token.kind()) => tokens.push(token),
            rowan::NodeOrToken::Token(_) => {}
        }
    }
}

fn first_significant_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    for element in node.children_with_tokens() {
        match element {
            rowan::NodeOrToken::Node(node) => {
                if let Some(token) = first_significant_token(&node) {
                    return Some(token);
                }
            }
            rowan::NodeOrToken::Token(token) if !is_trivia(token.kind()) => return Some(token),
            rowan::NodeOrToken::Token(_) => {}
        }
    }
    None
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
}

fn is_operator_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::Star | SyntaxKind::Slash
    )
}
