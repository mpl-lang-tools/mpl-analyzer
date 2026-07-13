//! Semantic validation for lowered HIR.
//!
//! Syntax diagnostics are passed through, then this module validates the HIR
//! model for unsupported directives, missing sources/functions, unknown
//! functions, deprecated constructs, and invalid parameters. Diagnostics carry
//! source ranges copied during lowering.

use serde::Serialize;

use mpl_syntax::{Parse, SourceFileNode, TextRange};

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
    let file = lower(parse);
    validate_hir(&file, diagnostics)
}

fn validate_hir(file: &HirFile, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut validator = Validator { diagnostics };
    validator.validate_file(file);
    validator.diagnostics
}

fn syntax_diagnostics(parse: &Parse<SourceFileNode>) -> Vec<Diagnostic> {
    parse
        .diagnostics()
        .iter()
        .map(|diag| Diagnostic {
            severity: Severity::Error,
            message: diag.message.clone(),
            range: diag.range,
        })
        .collect()
}

struct Validator {
    diagnostics: Vec<Diagnostic>,
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
        let name_text = directive
            .name
            .as_ref()
            .map(|name| name.text.as_str())
            .unwrap_or_default();

        if name_text != "custom_unit" {
            if name_text.is_empty() {
                self.error(
                    directive.range,
                    "unsupported set directive; only `custom_unit` is supported",
                );
            } else {
                self.error(
                    directive.range,
                    format!(
                        "unsupported set directive `{name_text}`; only `custom_unit` is supported"
                    ),
                );
            }
        }

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
                if pipe.keyword.as_deref() == Some("filter") {
                    self.hint(pipe.range, "`filter` is deprecated; use `where`");
                }

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
                if let Some(keyword) = &pipe.keyword {
                    self.error(pipe.range, format!("unknown pipe keyword `{keyword}`"));
                } else {
                    self.error(pipe.range, "unknown pipe keyword");
                }
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
                format!("unknown {} function `{}`", kind.name(), function.name.text),
            );
        }

        for arg in &function.args {
            self.validate_expr(arg);
        }
    }

    fn validate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Param { name, .. } => {
                if !stdlib::is_builtin_param(&name.text) {
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

    fn hint(&mut self, range: TextRange, message: impl Into<String>) {
        self.push(Severity::Hint, range, message);
    }

    fn push(&mut self, severity: Severity, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            severity,
            message: message.into(),
            range,
        });
    }
}
