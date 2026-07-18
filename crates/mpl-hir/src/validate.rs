//! Semantic validation for lowered HIR.
//!
//! Syntax diagnostics are passed through, then this module validates the HIR
//! model for unsupported directives, missing sources/functions, unknown
//! functions, deprecated constructs, and invalid parameters. Diagnostics carry
//! source ranges copied during lowering.

use std::collections::HashSet;

use serde::Serialize;

use mpl_syntax::{Parse, SourceFileNode, TextRange, TokenKind, lex};

use crate::lower::lower;
use crate::model::{
    AlignPipe, Assignment, BucketPipe, ComputeQuery, ComputeRule, Directive, Expr, FunctionCall,
    FunctionPipe, GroupPipe, HirFile, Pipe, Query, SimpleQuery, Source,
};
use crate::stdlib::{self, FunctionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub range: TextRange,
}

pub fn validate(parse: &Parse<SourceFileNode>) -> Vec<Diagnostic> {
    let diagnostics = syntax_diagnostics(parse);
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    let source = parse.syntax().text().to_string();
    let declared_params = declared_params(&source);
    let file = lower(parse);
    validate_hir(&file, diagnostics, declared_params)
}

fn validate_hir(
    file: &HirFile,
    diagnostics: Vec<Diagnostic>,
    declared_params: HashSet<String>,
) -> Vec<Diagnostic> {
    let mut validator = Validator {
        diagnostics,
        declared_params,
    };
    validator.validate_file(file);
    validator.diagnostics
}

fn syntax_diagnostics(parse: &Parse<SourceFileNode>) -> Vec<Diagnostic> {
    parse
        .diagnostics()
        .iter()
        .take(1)
        .map(|diag| Diagnostic {
            severity: Severity::Error,
            message: "MPL syntax error: unexpected token or operation".to_string(),
            range: diag.range,
        })
        .collect()
}

struct Validator {
    diagnostics: Vec<Diagnostic>,
    declared_params: HashSet<String>,
}

impl Validator {
    fn validate_file(&mut self, file: &HirFile) {
        for directive in &file.directives {
            self.validate_directive(directive);
        }

        if let Some(query) = &file.query {
            self.validate_query(query);
        }
    }

    fn validate_directive(&mut self, directive: &Directive) {
        if let Some(value) = &directive.value {
            self.validate_expr(value);
        }
    }

    fn validate_query(&mut self, query: &Query) {
        match query {
            Query::Simple(query) => self.validate_simple_query(query),
            Query::Compute(query) => self.validate_compute_query(query),
        }
    }

    fn validate_simple_query(&mut self, query: &SimpleQuery) {
        if let Some(source) = &query.source {
            self.validate_source(source);
        } else {
            self.error(query.range, "missing source");
        }

        for pipe in &query.pipes {
            self.validate_pipe(pipe);
        }
    }

    fn validate_compute_query(&mut self, query: &ComputeQuery) {
        for input in &query.inputs {
            self.validate_query(input);
        }

        if let Some(rule) = &query.rule {
            self.validate_compute_rule(rule);
        } else {
            self.error(query.range, "missing compute rule");
        }

        for pipe in &query.pipes {
            self.validate_pipe(pipe);
        }
    }

    fn validate_source(&mut self, source: &Source) {
        if source.dataset.is_none() {
            self.error(source.range, "missing source dataset");
        }

        if source.metric.is_none() {
            self.error(source.range, "missing source metric");
        }
    }

    fn validate_pipe(&mut self, pipe: &Pipe) {
        match pipe {
            Pipe::Where(pipe) => {
                for predicate in &pipe.predicates {
                    self.validate_expr(predicate);
                }
            }
            Pipe::Map(pipe) => self.validate_function_pipe(FunctionKind::Map, pipe),
            Pipe::Align(pipe) => self.validate_align_pipe(pipe),
            Pipe::Group(pipe) => self.validate_group_pipe(pipe),
            Pipe::Bucket(pipe) => self.validate_bucket_pipe(pipe),
            Pipe::Extend(pipe) => {
                for assignment in &pipe.assignments {
                    self.validate_assignment(assignment);
                }
            }
            Pipe::As(_) => {}
            Pipe::Unknown(pipe) => {
                let _ = pipe;
            }
        }
    }

    fn validate_function_pipe(&mut self, kind: FunctionKind, pipe: &FunctionPipe) {
        if let Some(function) = &pipe.function {
            self.validate_function_call(kind, function);
        }

        for expr in &pipe.exprs {
            self.validate_expr(expr);
        }
    }

    fn validate_align_pipe(&mut self, pipe: &AlignPipe) {
        if let Some(window) = &pipe.window {
            self.validate_expr(window);
        }

        self.validate_using_function(FunctionKind::Align, pipe.function.as_ref(), pipe.range);
    }

    fn validate_group_pipe(&mut self, pipe: &GroupPipe) {
        self.validate_using_function(FunctionKind::Group, pipe.function.as_ref(), pipe.range);
    }

    fn validate_bucket_pipe(&mut self, pipe: &BucketPipe) {
        if let Some(window) = &pipe.window {
            self.validate_expr(window);
        }

        self.validate_using_function(FunctionKind::Bucket, pipe.function.as_ref(), pipe.range);
    }

    fn validate_compute_rule(&mut self, rule: &ComputeRule) {
        self.validate_using_function(FunctionKind::Compute, rule.function.as_ref(), rule.range);
    }

    fn validate_assignment(&mut self, assignment: &Assignment) {
        if let Some(value) = &assignment.value {
            self.validate_expr(value);
        }
    }

    fn validate_using_function(
        &mut self,
        kind: FunctionKind,
        function: Option<&FunctionCall>,
        range: TextRange,
    ) {
        if let Some(function) = function {
            self.validate_function_call(kind, function);
        } else {
            self.error(
                range,
                format!("missing `using` function for {}", kind.name()),
            );
        }
    }

    fn validate_function_call(&mut self, kind: FunctionKind, function: &FunctionCall) {
        if !stdlib::is_function(kind, &function.name.text) {
            self.error(
                function.name.range,
                format!(
                    "Unsupported {} function: {}",
                    kind.name(),
                    function.name.text
                ),
            );
        }

        for arg in &function.args {
            self.validate_expr(arg);
        }
    }

    fn validate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Param { name, .. } => {
                if !stdlib::is_builtin_param(&name.text)
                    && !self.declared_params.contains(&normalise_param(&name.text))
                {
                    self.error(name.range, format!("unknown parameter `{}`", name.text));
                }
            }
            Expr::Call { call, .. } => {
                for arg in &call.args {
                    self.validate_expr(arg);
                }
            }
            Expr::Not {
                expr: Some(expr), ..
            }
            | Expr::Paren {
                expr: Some(expr), ..
            } => self.validate_expr(expr),
            Expr::Compare { rhs, .. } => {
                if let Some(rhs) = rhs {
                    self.validate_expr(rhs);
                }
            }
            _ => {}
        }
    }

    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.push(Severity::Error, range, message);
    }

    fn push(&mut self, severity: Severity, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            severity,
            message: message.into(),
            range,
        });
    }
}

fn declared_params(input: &str) -> HashSet<String> {
    let tokens = lex(input);
    let mut params = HashSet::new();
    let mut iter = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))
        .peekable();

    while let Some(token) = iter.next() {
        if token.kind == TokenKind::Keyword && token.text == "param" {
            if let Some(param) = iter.next_if(|token| token.kind == TokenKind::Param) {
                params.insert(normalise_param(&param.text));
            }
        }
    }

    params
}

fn normalise_param(text: &str) -> String {
    if let Some(inner) = text
        .strip_prefix("$`")
        .and_then(|text| text.strip_suffix('`'))
    {
        format!("${inner}")
    } else {
        text.to_string()
    }
}
