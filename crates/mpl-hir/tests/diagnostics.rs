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

    fn help(&self) -> Option<Box<dyn Display + '_>> {
        self.diagnostic
            .help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn Display>)
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
        for action in &diagnostic.actions {
            writeln!(
                snapshot,
                "   Fix: {} -> {:?}",
                action.title, action.replacement
            )
            .unwrap();
        }
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
  | map filter::gt(150)
  | map is::ge(0.95)
  | align to $__interval using avg
  | bucket by host using histogram(count)
"#,
    ));
}

#[test]
fn public_example_constructs_are_accepted() {
    insta::assert_snapshot!(snapshot(
        r#"param $dataset: Dataset;
param $duration: Duration;
param $tag: Option<string>;
set no_arg;
set string_arg = "Hello, World!";

$dataset:metric
| ifdef($tag) { where __tag == $tag } else { where __tag == "default" }
| sample
| where i1 == inf and i2 == -inf and i3 == +inf
| align to $duration over 7d using avg
"#,
    ));
}

#[test]
fn real_indented_sample_is_clean() {
    insta::assert_snapshot!(snapshot(
        r#"test:http_requests_total
  | where status == 500
  | align to 5m using avg
"#,
    ));
}

#[test]
fn warns_about_lowercase_duration_param_type() {
    insta::assert_snapshot!(snapshot(
        "param $window: duration;\nprod:requests | align to $window using avg",
    ));
}

#[test]
fn warns_about_declared_param_using_system_prefix() {
    insta::assert_snapshot!(snapshot(
        "param $__window: Duration;\nprod:requests | align to $__window using avg",
    ));
}

#[test]
fn warns_to_replace_deprecated_filter() {
    insta::assert_snapshot!(snapshot("prod:requests | filter status == 500",));
}

#[test]
fn hints_to_remove_unnecessary_identifier_backticks() {
    insta::assert_snapshot!(snapshot("prod:requests | where `status` == 500",));
}

#[test]
fn keeps_backticks_required_by_identifier_syntax() {
    insta::assert_snapshot!(snapshot("prod:requests | where `http.status` == 500",));
}

#[test]
fn rejects_invalid_time_unit_like_mplc() {
    insta::assert_snapshot!(snapshot("dataset:metric | align to 5x using avg"));
}

#[test]
fn rejects_unknown_pipe_keyword_like_mplc() {
    insta::assert_snapshot!(snapshot("dataset:metric | fflter tag == \"value\"",));
}

#[test]
fn rejects_unresolved_source_parameter_like_mplc() {
    insta::assert_snapshot!(snapshot("$does_not_exist:bar"));
}

#[test]
fn rejects_cumulative_histogram_without_conversion_like_mplc() {
    insta::assert_snapshot!(snapshot(
        "test:http_request_duration_seconds_bucket\n\
         | where code == #/[123]../\n\
         | bucket by method, path to 5m using interpolate_cumulative_histogram(sum, 0.90, 0.99)",
    ));
}

#[test]
fn rejects_delta_histogram_conversion_like_mplc() {
    insta::assert_snapshot!(snapshot(
        "test:http_request_duration_seconds_bucket\n\
         | where code == #/[123]../\n\
         | bucket by method, path to 5m using interpolate_delta_histogram(rate, 0.90, 0.99)",
    ));
}

#[test]
fn accepts_join_like_mplc() {
    insta::assert_snapshot!(snapshot(
        "test:kube_pod_status_ready\n\
         | group by pod using sum\n\
         | join created_by_kind from test:kube_pod_info by pod",
    ));
}

#[test]
fn accepts_replace_like_mplc() {
    insta::assert_snapshot!(snapshot(
        "test:container_cpu_usage_seconds_total\n\
         | replace service = pod ~ #s/(.+)-.+-.+/$1/\n\
         | group by service using max",
    ));
}

#[test]
fn rejects_join_without_from_like_mplc() {
    insta::assert_snapshot!(snapshot("test:requests | join host test:metadata by host",));
}

#[test]
fn rejects_replace_without_pattern_like_mplc() {
    insta::assert_snapshot!(snapshot("test:requests | replace service = pod ~"));
}
