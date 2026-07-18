//! Snapshot coverage for syntax-layer behavior.
//!
//! These tests render source, CST output, diagnostics, formatter output, and
//! wrapper traversal into self-contained snapshots so grammar and recovery
//! changes can be reviewed without opening the test source.

use display_tree::{AsTree, CharSet, DisplayTree, Style, StyleBuilder};
use mpl_syntax::{
    AstNode, FunctionCallNode, NameRefNode, SyntaxElement, SyntaxNode, SyntaxToken, format_source,
    parse,
};
use std::error::Error;
use std::fmt::{self, Write};

const MIETTE_ALL_CONTEXT_LINES: usize = 10_000;

struct CstTree {
    label: String,
    children: Vec<CstTree>,
}

struct SyntaxSnapshotDiagnostic {
    source: String,
    diagnostic: mpl_syntax::SyntaxDiagnostic,
}

impl fmt::Debug for SyntaxSnapshotDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxSnapshotDiagnostic")
            .field("message", &self.diagnostic.message)
            .finish()
    }
}

impl fmt::Display for SyntaxSnapshotDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.diagnostic.message)
    }
}

impl Error for SyntaxSnapshotDiagnostic {}

impl miette::Diagnostic for SyntaxSnapshotDiagnostic {
    fn severity(&self) -> Option<miette::Severity> {
        Some(miette::Severity::Error)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(miette::LabeledSpan::at(
            span(&self.diagnostic),
            self.diagnostic.message.clone(),
        ))))
    }
}

impl DisplayTree for CstTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, style: Style) -> fmt::Result {
        writeln!(f, "{}", style.leaf_style.apply(&self.label))?;
        self.fmt_children(f, style, "")
    }
}

impl CstTree {
    fn fmt_children(&self, f: &mut fmt::Formatter<'_>, style: Style, prefix: &str) -> fmt::Result {
        for (index, child) in self.children.iter().enumerate() {
            let is_last = index + 1 == self.children.len();
            let connector = if is_last {
                style.char_set.end_connector
            } else {
                style.char_set.connector
            };
            let branch = format!(
                "{connector}{} ",
                style
                    .char_set
                    .horizontal
                    .to_string()
                    .repeat(style.indentation as usize)
            );
            writeln!(
                f,
                "{}{}{}",
                prefix,
                style.branch_style.apply(&branch),
                style.leaf_style.apply(&child.label),
            )?;

            let continuation = if is_last {
                " ".repeat(style.indentation as usize + 2)
            } else {
                format!(
                    "{}{}",
                    style.char_set.vertical,
                    " ".repeat(style.indentation as usize + 1)
                )
            };
            child.fmt_children(f, style, &format!("{prefix}{continuation}"))?;
        }
        Ok(())
    }
}

fn write_block(snapshot: &mut String, content: &str) {
    write!(snapshot, "{content}").unwrap();
    if !content.ends_with('\n') {
        writeln!(snapshot).unwrap();
    }
}

fn write_section(snapshot: &mut String, title: &str) {
    writeln!(snapshot, "{}:", title.to_uppercase()).unwrap();
    writeln!(snapshot).unwrap();
}

fn write_subsection(snapshot: &mut String, title: &str) {
    writeln!(snapshot, "{}:", title.to_uppercase()).unwrap();
    writeln!(snapshot).unwrap();
}

fn format_snapshot(input: &str, output: &str) -> String {
    let mut snapshot = String::new();
    write_section(&mut snapshot, "INPUT");
    write_block(&mut snapshot, input);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "OUTPUT");
    write_block(&mut snapshot, output);
    snapshot
}

fn wrapper_snapshot(
    input: &str,
    directives: usize,
    queries: usize,
    names: &[String],
    functions: &[String],
    roundtrip: &str,
) -> String {
    let mut snapshot = String::new();
    write_section(&mut snapshot, "INPUT");
    write_block(&mut snapshot, input);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "COUNTS");
    writeln!(snapshot, "directives: {directives}").unwrap();
    writeln!(snapshot, "queries: {queries}").unwrap();
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "NAMES");
    write_list(&mut snapshot, names);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "FUNCTIONS");
    write_list(&mut snapshot, functions);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "ROUNDTRIP");
    write_block(&mut snapshot, roundtrip);
    snapshot
}

fn write_list(snapshot: &mut String, items: &[String]) {
    if items.is_empty() {
        writeln!(snapshot, "(empty)").unwrap();
        return;
    }

    for item in items {
        writeln!(snapshot, "  {item}").unwrap();
    }
}

fn display_cst(node: &SyntaxNode) -> String {
    AsTree::new(&cst_node(node))
        .indentation(2)
        .char_set(CharSet::SINGLE_LINE_CURVED)
        .to_string()
}

fn cst_node(node: &SyntaxNode) -> CstTree {
    CstTree {
        label: format!("{:?}", node.kind()),
        children: node.children_with_tokens().map(cst_element).collect(),
    }
}

fn cst_element(element: SyntaxElement) -> CstTree {
    match element {
        SyntaxElement::Node(node) => cst_node(&node),
        SyntaxElement::Token(token) => cst_token(&token),
    }
}

fn cst_token(token: &SyntaxToken) -> CstTree {
    CstTree {
        label: format!("{:?} {:?}", token.kind(), token.text()),
        children: Vec::new(),
    }
}

fn render_diagnostic(input: &str, diagnostic: mpl_syntax::SyntaxDiagnostic) -> String {
    let diagnostic = SyntaxSnapshotDiagnostic {
        source: input.to_string(),
        diagnostic,
    };
    let handler =
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .with_context_lines(MIETTE_ALL_CONTEXT_LINES)
            .with_width(100)
            .with_links(false);
    let mut rendered = String::new();
    handler
        .render_report(&mut rendered, &diagnostic)
        .expect("miette report should render");
    rendered.replace("\u{261e} ", "").replace('\u{261e}', "")
}

fn span(diagnostic: &mpl_syntax::SyntaxDiagnostic) -> miette::SourceSpan {
    let len = diagnostic.range.end.saturating_sub(diagnostic.range.start);
    (diagnostic.range.start, len).into()
}

fn syntax_snapshot(input: &str, cst: &str, diagnostics: &[mpl_syntax::SyntaxDiagnostic]) -> String {
    let mut snapshot = String::new();
    write_section(&mut snapshot, "INPUT");
    write_block(&mut snapshot, input);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "CST");
    write_block(&mut snapshot, cst);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "DIAGNOSTICS");

    if diagnostics.is_empty() {
        writeln!(snapshot, "No diagnostics.").unwrap();
        return snapshot;
    }

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        writeln!(snapshot, "{}. Error: {}", index + 1, diagnostic.message).unwrap();
    }

    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "REPORTS");
    for (index, diagnostic) in diagnostics.iter().cloned().enumerate() {
        let rendered = render_diagnostic(input, diagnostic);
        if index > 0 {
            writeln!(snapshot).unwrap();
        }
        write_subsection(&mut snapshot, &format!("REPORT {}", index + 1));
        write_block(&mut snapshot, &rendered);
    }
    snapshot
}

#[test]
fn parses_public_v1_query_shape() {
    let input = r#"set custom_unit = "ms";
http:request_duration[5m] as latency
| where service == "api" and not status is string
| map fill::const(0)
| align to $__interval using avg
| group by service, region using sum
| bucket by status to 1m using histogram("le")
| extend slo = 99.9, matcher = #/api\/v1/
| as api_latency
"#;

    let parse = parse(input);
    insta::assert_snapshot!(syntax_snapshot(
        input,
        &display_cst(parse.syntax()),
        parse.diagnostics(),
    ));
}

#[test]
fn parses_indented_public_sample() {
    let input = r#"test:http_requests_total
  | filter status == 500
  | align to 5m using avg
"#;

    let parse = parse(input);
    insta::assert_snapshot!(syntax_snapshot(
        input,
        &display_cst(parse.syntax()),
        parse.diagnostics(),
    ));
}

#[test]
fn parses_compute_query() {
    let input =
        r#"(cpu:usage[1h] | map rate, mem:rss[1h]) | compute total using avg | as total_usage"#;

    let parse = parse(input);
    insta::assert_snapshot!(syntax_snapshot(
        input,
        &display_cst(parse.syntax()),
        parse.diagnostics(),
    ));
}

#[test]
fn parses_extended_public_examples() {
    let input = r#"param $dataset: Dataset;
param $duration: Duration;
param $tag: Option<string>;
set no_arg;
set string_arg = "Hello, World!";

$dataset:metric
| ifdef($tag) { where __tag == $tag } else { where __tag == "default" }
| sample
| where i1 == inf and i2 == -inf and i3 == +inf
| align to $duration over 7d using avg
"#;

    let parse = parse(input);
    insta::assert_snapshot!(syntax_snapshot(
        input,
        &display_cst(parse.syntax()),
        parse.diagnostics(),
    ));
}

#[test]
fn reports_recovery_diagnostics() {
    let input = r#"set custom_unit = "ms";
http:request_duration[5m]
| where service "api"
| extend label
"#;

    let parse = parse(input);
    insta::assert_snapshot!(syntax_snapshot(
        input,
        &display_cst(parse.syntax()),
        parse.diagnostics(),
    ));
}

#[test]
fn formats_deterministically_and_preserves_comments() {
    let input = r#"// leading
set   custom_unit="ms"; http:request_duration [ 5m ] as latency |where service=="api"|map fill::const(0)// tail
| extend slo=99.9,matcher=#/api/
"#;

    insta::assert_snapshot!(format_snapshot(input, &format_source(input)));
}

#[test]
fn snapshots_lossless_cst_tree() {
    let input = r#"// leading
set custom_unit = "ms";
http:request_duration[5m] as latency
| where service == "api"
| map fill::const(0)
"#;

    let parse = parse(input);
    insta::assert_snapshot!(syntax_snapshot(
        input,
        &display_cst(parse.syntax()),
        parse.diagnostics(),
    ));
}

#[test]
fn exposes_ast_wrapper_traversal() {
    let input = r#"http:request_duration[5m] as latency
| where service == "api"
| map fill::const(0)
"#;

    let parse = parse(input);
    let names: Vec<_> = parse
        .syntax()
        .descendants()
        .filter_map(NameRefNode::cast)
        .map(|node| node.text())
        .collect();
    let functions: Vec<_> = parse
        .syntax()
        .descendants()
        .filter_map(FunctionCallNode::cast)
        .filter_map(|node| node.name().map(|name| name.text()))
        .collect();

    insta::assert_snapshot!(wrapper_snapshot(
        input,
        parse.tree().directives().count(),
        parse.tree().queries().count(),
        &names,
        &functions,
        &parse.syntax().text().to_string(),
    ));
}
