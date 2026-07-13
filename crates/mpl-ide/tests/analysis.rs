//! Snapshot coverage for IDE-facing analysis features.
//!
//! These tests exercise completions, hover, signature help, and formatting
//! through the public `mpl-ide` API. Cursor snapshots include source, byte
//! offset, and output so editor behavior can be accepted from the snapshot.

use mpl_ide::{CompletionItem, Hover, SignatureHelp, completions, format, hover, signature_help};
use std::error::Error;
use std::fmt;
use std::fmt::Write;

const CURSOR_MARKER: &str = "$0";
const CURSOR_DISPLAY: &str = "█";
const MIETTE_ALL_CONTEXT_LINES: usize = 10_000;

struct IdeReport {
    source: String,
    message: String,
    label: String,
    help: Option<String>,
    offset: usize,
    len: usize,
}

impl fmt::Debug for IdeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdeReport")
            .field("message", &self.message)
            .field("label", &self.label)
            .finish()
    }
}

impl fmt::Display for IdeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for IdeReport {}

impl miette::Diagnostic for IdeReport {
    fn severity(&self) -> Option<miette::Severity> {
        Some(miette::Severity::Advice)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(Box::new)
            .map(|help| help as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(miette::LabeledSpan::at(
            (self.offset, self.len),
            self.label.clone(),
        ))))
    }
}

fn cursor(input: &str) -> (String, usize) {
    let offset = input.find(CURSOR_MARKER).expect("missing $0 cursor marker");
    let mut text = input.to_string();
    text.replace_range(offset..offset + CURSOR_MARKER.len(), "");
    (text, offset)
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

fn write_case_section(snapshot: &mut String, title: &str) {
    writeln!(snapshot, "{}", title.to_uppercase()).unwrap();
    writeln!(snapshot).unwrap();
}

fn write_cursor_case_header(
    snapshot: &mut String,
    case_index: usize,
    name: &str,
    source_with_cursor: &str,
) {
    let display_source = source_with_cursor.replace(CURSOR_MARKER, CURSOR_DISPLAY);
    write_case_section(snapshot, &format!("CASE {case_index}: {name}"));
    write_subsection(snapshot, "SOURCE");
    write_block(snapshot, &display_source);
    writeln!(snapshot).unwrap();
}

fn render_report(report: IdeReport) -> String {
    let handler =
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .with_context_lines(MIETTE_ALL_CONTEXT_LINES)
            .with_width(100)
            .with_links(false);
    let mut rendered = String::new();
    handler
        .render_report(&mut rendered, &report)
        .expect("miette report should render");
    rendered.replace("\u{261e} ", "").replace('\u{261e}', "")
}

fn cursor_span(source: &str, offset: usize) -> (usize, usize) {
    if source.is_empty() {
        return (0, 0);
    }

    (offset.min(source.len().saturating_sub(1)), 1)
}

fn split_title_body(contents: &str) -> (String, Option<String>) {
    let trimmed = contents.trim();
    let Some((title, body)) = trimmed.split_once("\n\n") else {
        return (trimmed.to_string(), None);
    };

    let body = body.trim();
    (
        title.trim().to_string(),
        (!body.is_empty()).then(|| body.to_string()),
    )
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

fn completions_snapshot(cases: &[(&str, &str)]) -> String {
    let mut snapshot = String::new();
    for (index, (name, source_with_cursor)) in cases.iter().enumerate() {
        if index > 0 {
            writeln!(snapshot).unwrap();
        }

        let (source, offset) = cursor(source_with_cursor);
        let output = completions(&source, offset);
        write_cursor_case_header(&mut snapshot, index + 1, name, source_with_cursor);
        write_subsection(&mut snapshot, "COMPLETIONS");
        write_completion_report(&mut snapshot, &source, offset, &output);
    }
    snapshot
}

fn write_completion_report(
    snapshot: &mut String,
    source: &str,
    offset: usize,
    items: &[CompletionItem],
) {
    let labels: Vec<_> = items.iter().map(|item| item.label.clone()).collect();
    let (offset, len) = cursor_span(source, offset);
    let rendered = render_report(IdeReport {
        source: source.to_string(),
        message: "completions".into(),
        label: format!("{} completion item(s)", labels.len()),
        help: (!labels.is_empty()).then(|| labels.join("\n")),
        offset,
        len,
    });
    write_block(snapshot, &rendered);
}

fn hovers_snapshot(cases: &[(&str, &str)]) -> String {
    let mut snapshot = String::new();
    for (index, (name, source_with_cursor)) in cases.iter().enumerate() {
        if index > 0 {
            writeln!(snapshot).unwrap();
        }

        let (source, offset) = cursor(source_with_cursor);
        let output = hover(&source, offset);
        write_cursor_case_header(&mut snapshot, index + 1, name, source_with_cursor);
        write_subsection(&mut snapshot, "HOVER");
        write_hover(&mut snapshot, &source, output.as_ref());
    }
    snapshot
}

fn write_hover(snapshot: &mut String, source: &str, hover: Option<&Hover>) {
    let Some(hover) = hover else {
        writeln!(snapshot, "None").unwrap();
        return;
    };

    let (label, help) = split_title_body(&hover.contents);
    let rendered = render_report(IdeReport {
        source: source.to_string(),
        message: "hover".into(),
        label,
        help,
        offset: hover.range.start,
        len: hover.range.end.saturating_sub(hover.range.start),
    });
    write_block(snapshot, &rendered);
}

fn signature_snapshot(cases: &[(&str, &str)]) -> String {
    let mut snapshot = String::new();
    for (index, (name, source_with_cursor)) in cases.iter().enumerate() {
        if index > 0 {
            writeln!(snapshot).unwrap();
        }

        let (source, offset) = cursor(source_with_cursor);
        let output = signature_help(&source, offset);
        write_cursor_case_header(&mut snapshot, index + 1, name, source_with_cursor);
        write_subsection(&mut snapshot, "SIGNATURE HELP");
        write_signature_help(&mut snapshot, &source, output.as_ref());
    }
    snapshot
}

fn write_signature_help(snapshot: &mut String, source: &str, signature: Option<&SignatureHelp>) {
    let Some(signature) = signature else {
        writeln!(snapshot, "None").unwrap();
        return;
    };

    let rendered = render_report(IdeReport {
        source: source.to_string(),
        message: "signature help".into(),
        label: signature.signature.clone(),
        help: None,
        offset: signature.range.start,
        len: signature.range.end.saturating_sub(signature.range.start),
    });
    write_block(snapshot, &rendered);
}

#[test]
fn completes_pipe_keywords() {
    insta::assert_snapshot!(completions_snapshot(&[(
        "pipe_keywords",
        "from prod:requests | $0",
    )]));
}

#[test]
fn completes_functions_by_using_context() {
    insta::assert_snapshot!(completions_snapshot(&[
        ("align", "from prod:requests | align $__interval using $0"),
        ("group", "from prod:requests | group by host using $0"),
        ("bucket", "from prod:requests | bucket by le using $0"),
        ("compute", "compute total using $0"),
        ("map_using", "from prod:requests | map using $0"),
        ("map_direct", "from prod:requests | map $0"),
    ]));
}

#[test]
fn completes_source_params_and_comparison_literals() {
    insta::assert_snapshot!(completions_snapshot(&[
        ("source_start", "$0"),
        ("param", "from prod:requests | align $0"),
        ("comparison", "from prod:requests | where status == $0"),
    ]));
}

#[test]
fn hovers_functions_and_keywords() {
    insta::assert_snapshot!(hovers_snapshot(&[
        ("function", "from prod:requests | map fill::$0const(0)"),
        (
            "keyword",
            "from prod:requests | $0align $__interval using avg",
        ),
    ]));
}

#[test]
fn signature_help_for_functions_and_keywords() {
    insta::assert_snapshot!(signature_snapshot(&[
        ("function", "from prod:requests | map fill::const($0)"),
        ("keyword", "from prod:requests | $0group by host using sum"),
    ]));
}

#[test]
fn formats_from_syntax_tokens() {
    let input = "from prod:requests|align $__interval using avg;";
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}
