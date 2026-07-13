//! Snapshot coverage for HIR semantic diagnostics.
//!
//! Each snapshot includes the MPL input and resulting diagnostics so validation
//! behavior can be reviewed from the snapshot alone, including syntax
//! diagnostic pass-through and semantic checks.

use mpl_hir::{Diagnostic, validate};
use mpl_syntax::parse_syntax;
use std::error::Error;
use std::fmt::{self, Display, Write};

const MIETTE_ALL_CONTEXT_LINES: usize = 10_000;

struct SnapshotDiagnostic {
    source: String,
    diagnostic: Diagnostic,
}

impl Display for SnapshotDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.diagnostic.message)
    }
}

impl fmt::Debug for SnapshotDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnapshotDiagnostic")
            .field("message", &self.diagnostic.message)
            .finish()
    }
}

impl Error for SnapshotDiagnostic {}

impl miette::Diagnostic for SnapshotDiagnostic {
    fn severity(&self) -> Option<miette::Severity> {
        Some(match self.diagnostic.severity {
            mpl_hir::Severity::Error => miette::Severity::Error,
            mpl_hir::Severity::Warning => miette::Severity::Warning,
            mpl_hir::Severity::Hint => miette::Severity::Advice,
        })
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

fn snapshot(input: &str) -> String {
    let parse = parse_syntax(input);
    let diagnostics = validate(&parse);
    let mut snapshot = String::new();
    write_section(&mut snapshot, "INPUT");
    write_block(&mut snapshot, input);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "DIAGNOSTICS");

    if diagnostics.is_empty() {
        writeln!(snapshot, "No diagnostics.").unwrap();
        return snapshot;
    }

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        writeln!(
            snapshot,
            "{}. {:?}: {}",
            index + 1,
            diagnostic.severity,
            diagnostic.message
        )
        .unwrap();
    }

    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "REPORTS");
    for (index, diagnostic) in diagnostics.into_iter().enumerate() {
        let rendered = render_report(input, diagnostic);
        if index > 0 {
            writeln!(snapshot).unwrap();
        }
        write_subsection(&mut snapshot, &format!("REPORT {}", index + 1));
        write_block(&mut snapshot, &rendered);
    }
    snapshot
}

fn render_report(input: &str, diagnostic: Diagnostic) -> String {
    let diagnostic = SnapshotDiagnostic {
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

fn span(diagnostic: &Diagnostic) -> miette::SourceSpan {
    let len = diagnostic.range.end.saturating_sub(diagnostic.range.start);
    (diagnostic.range.start, len).into()
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

#[test]
fn syntax_diagnostics_pass_through() {
    insta::assert_snapshot!(snapshot("dataset:"));
}

#[test]
fn semantic_diagnostics() {
    insta::assert_snapshot!(snapshot(
        r#"set timezone = "UTC";
| filter status == 500
| sample
| map bogus
| align to $__interval
| extend step = $custom
"#,
    ));
}

#[test]
fn unknown_functions_are_checked_by_category() {
    insta::assert_snapshot!(snapshot(
        r#"(dataset:metric
  | map avg
  | align using filter::lt
  | group by host using prom::rate
  | bucket using sum
) | compute total using rate
"#,
    ));
}

#[test]
fn known_functions_and_builtin_interval_are_accepted() {
    insta::assert_snapshot!(snapshot(
        r#"set custom_unit = "ms";
dataset:metric
  | map fill::const(0)
  | align to $__interval using avg
  | bucket by host using histogram
"#,
    ));
}

#[test]
fn real_indented_sample_only_reports_filter_hint() {
    insta::assert_snapshot!(snapshot(
        r#"test:http_requests_total
  | filter status == 500
  | align to 5m using avg
"#,
    ));
}
