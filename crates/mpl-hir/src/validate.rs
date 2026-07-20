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
    pub help: Option<String>,
    pub actions: Vec<DiagnosticAction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticAction {
    pub title: String,
    pub range: TextRange,
    pub replacement: String,
}

pub fn validate(parse: &Parse<SourceFileNode>) -> Vec<Diagnostic> {
    let diagnostics = syntax_diagnostics(parse);
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    let source = parse.syntax().text().to_string();
    let declared_params = declared_params(&source);
    let file = lower(parse);
    let mut diagnostics = validate_hir(&file, diagnostics, declared_params);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return diagnostics;
    }

    diagnostics.extend(source_diagnostics(&source));
    diagnostics
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
            help: None,
            actions: Vec::new(),
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
        if let Some(dataset) = &source.dataset {
            self.validate_parameter_name(dataset);
        } else {
            self.error(source.range, "missing source dataset");
        }

        if let Some(metric) = &source.metric {
            self.validate_parameter_name(metric);
        } else {
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
        if let Some(function) = &pipe.function {
            self.validate_bucket_function(function);
        }
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

    fn validate_bucket_function(&mut self, function: &FunctionCall) {
        match function.name.text.as_str() {
            "histogram" | "interpolate_delta_histogram" => {
                if function.args.is_empty() {
                    self.error(
                        function.name.range,
                        format!("missing bucket specifications for `{}`", function.name.text),
                    );
                }
                for arg in &function.args {
                    self.validate_bucket_spec(arg);
                }
            }
            "interpolate_cumulative_histogram" => {
                let Some((conversion, specs)) = function.args.split_first() else {
                    self.error(
                        function.name.range,
                        "missing histogram conversion method and bucket specifications",
                    );
                    return;
                };
                if !matches!(
                    conversion,
                    Expr::Name { name, .. } if matches!(name.text.as_str(), "rate" | "increase")
                ) {
                    self.error(
                        conversion.range(),
                        "expected histogram conversion method `rate` or `increase`",
                    );
                }
                if specs.is_empty() {
                    self.error(function.name.range, "missing bucket specifications");
                }
                for spec in specs {
                    self.validate_bucket_spec(spec);
                }
            }
            _ => {}
        }
    }

    fn validate_bucket_spec(&mut self, spec: &Expr) {
        let valid = match spec {
            Expr::Number { .. } => true,
            Expr::Name { name, .. } => {
                matches!(name.text.as_str(), "count" | "avg" | "sum" | "min" | "max")
            }
            _ => false,
        };
        if !valid {
            self.error(
                spec.range(),
                "expected bucket specification `count`, `avg`, `sum`, `min`, `max`, or a number",
            );
        }
    }

    fn validate_parameter_name(&mut self, name: &crate::model::NameRef) {
        if name.text.starts_with('$')
            && !stdlib::is_builtin_param(&name.text)
            && !self.declared_params.contains(&normalise_param(&name.text))
        {
            self.error(name.range, format!("unknown parameter `{}`", name.text));
        }
    }

    fn validate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Param { name, .. } => {
                self.validate_parameter_name(name);
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
            Expr::Compare { rhs: Some(rhs), .. } => self.validate_expr(rhs),
            Expr::Compare { rhs: None, .. } => {}
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
            help: None,
            actions: Vec::new(),
        });
    }
}

fn source_diagnostics(source: &str) -> Vec<Diagnostic> {
    let tokens = lex(source);
    let significant = tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::Eof
            )
        })
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut hints = Vec::new();

    for (index, token) in significant.iter().enumerate() {
        if token.kind == TokenKind::Keyword
            && token.text == "filter"
            && index > 0
            && matches!(
                significant[index - 1].kind,
                TokenKind::Pipe | TokenKind::LBrace
            )
        {
            hints.push(Diagnostic {
                severity: Severity::Hint,
                message: "Consider using `where` instead of `filter`".to_string(),
                range: token.range,
                help: Some("`filter` is deprecated; `where` is preferred".to_string()),
                actions: vec![DiagnosticAction {
                    title: "Replace with `where`".to_string(),
                    range: token.range,
                    replacement: "where".to_string(),
                }],
            });
        }

        if token.kind == TokenKind::EscapedIdent {
            push_unnecessary_escape(&mut hints, token.text.as_str(), token.range);
        } else if token.kind == TokenKind::Param
            && let Some(escaped) = token.text.strip_prefix('$')
        {
            let range = TextRange::new(token.range.start + 1, token.range.end);
            push_unnecessary_escape(&mut hints, escaped, range);
        }
    }

    for declaration in significant.split_inclusive(|token| token.kind == TokenKind::Semicolon) {
        if declaration
            .first()
            .is_none_or(|token| token.kind != TokenKind::Keyword || token.text != "param")
        {
            continue;
        }

        if let Some((param, name)) = declaration
            .get(1)
            .filter(|token| token.kind == TokenKind::Param)
            .and_then(|param| param_name(&param.text).map(|name| (*param, name)))
            && name.starts_with("__")
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: format!(
                    "The param ${name} uses the `__` prefix reserved for system params"
                ),
                range: param.range,
                help: None,
                actions: Vec::new(),
            });
        }

        for token in declaration {
            if token.kind == TokenKind::Ident && token.text == "duration" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "`duration` is deprecated; use `Duration`".to_string(),
                    range: token.range,
                    help: Some(
                        "Param types use PascalCase: `Duration`, `Dataset`, `Regex`".to_string(),
                    ),
                    actions: vec![DiagnosticAction {
                        title: "Replace with `Duration`".to_string(),
                        range: token.range,
                        replacement: "Duration".to_string(),
                    }],
                });
            }
        }
    }

    diagnostics.extend(hints);
    diagnostics
}

fn push_unnecessary_escape(diagnostics: &mut Vec<Diagnostic>, escaped: &str, range: TextRange) {
    let Some(inner) = escaped
        .strip_prefix('`')
        .and_then(|text| text.strip_suffix('`'))
    else {
        return;
    };
    if inner.is_empty() || !is_plain_ident(inner) {
        return;
    }

    diagnostics.push(Diagnostic {
        severity: Severity::Hint,
        message: "Unnecessary backtick escaping".to_string(),
        range,
        help: Some(format!("`{inner}` is a valid unescaped identifier")),
        actions: vec![DiagnosticAction {
            title: "Remove backticks".to_string(),
            range,
            replacement: inner.to_string(),
        }],
    });
}

fn is_plain_ident(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn param_name(text: &str) -> Option<&str> {
    let name = text.strip_prefix('$')?;
    Some(
        name.strip_prefix('`')
            .and_then(|name| name.strip_suffix('`'))
            .unwrap_or(name),
    )
}

fn declared_params(input: &str) -> HashSet<String> {
    let tokens = lex(input);
    let mut params = HashSet::new();
    let mut iter = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))
        .peekable();

    while let Some(token) = iter.next() {
        if token.kind == TokenKind::Keyword
            && token.text == "param"
            && let Some(param) = iter.next_if(|token| token.kind == TokenKind::Param)
        {
            params.insert(normalise_param(&param.text));
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

#[cfg(test)]
mod tests {
    use mpl_syntax::parse_syntax;

    use super::{Severity, validate};

    #[test]
    fn detects_each_deprecated_filter_in_a_valid_query() {
        let diagnostics = validate(&parse_syntax("ds:metric | filter a == 1 | filter b == 2"));

        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.message.contains("`filter`"))
                .count(),
            2
        );
    }

    #[test]
    fn detects_deprecated_filter_inside_ifdef() {
        let diagnostics = validate(&parse_syntax(
            "param $tag: Option<string>;\nds:metric | ifdef($tag) { filter tag == $tag }",
        ));

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Hint && diagnostic.message.contains("`filter`")
        }));
    }

    #[test]
    fn suppresses_hints_when_parsing_fails() {
        let diagnostics = validate(&parse_syntax("ds: | filter `tag` == 1"));

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn detects_lowercase_duration_in_optional_param_type() {
        let diagnostics = validate(&parse_syntax(
            "param $window: Option<duration>;\nds:metric | align to 1m using avg",
        ));

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Warning && diagnostic.message.contains("`duration`")
        }));
    }
}
