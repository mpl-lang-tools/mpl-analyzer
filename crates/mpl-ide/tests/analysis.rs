//! Snapshot coverage for IDE-facing analysis features.
//!
//! These tests exercise completions, hover, signature help, and formatting
//! through the public `mpl-ide` API. Cursor positions are specified as a
//! zero-based line and Unicode-character offset.

use mpl_code_render::{Annotation, Position, Source};
use mpl_ide::{Hover, SignatureHelp, completions, format, hover, signature_help};
use std::fmt::Write;

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

fn render_cursor_source(source: &str, cursor: Position, label: &str) -> String {
    Source::new(source)
        .annotation(Annotation::point(cursor, label))
        .render()
        .expect("cursor should be a valid source position")
}

fn format_snapshot(input: &str, output: &str) -> String {
    let mut snapshot = String::new();
    write_section(&mut snapshot, "SOURCE");
    write_block(&mut snapshot, input);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "FORMATTED");
    write_block(&mut snapshot, output);
    snapshot
}

fn completions_snapshot(source: &str, cursor: Position, selected: &str) -> String {
    let mut snapshot = String::new();
    let offset = cursor
        .to_byte_offset(source)
        .expect("cursor should be a valid source position");
    let items = completions(source, offset);
    let selected_item = items
        .iter()
        .find(|item| item.label == selected)
        .unwrap_or_else(|| panic!("selected completion {selected:?} should be available"));
    let longest_label = items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    let labels = items
        .iter()
        .map(|item| {
            if item.label == selected {
                format!("{:<longest_label$} ←", item.label)
            } else {
                item.label.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    write_section(&mut snapshot, "SOURCE");
    let rendered_source = render_cursor_source(source, cursor, &labels);
    write_block(
        &mut snapshot,
        rendered_source
            .strip_prefix('\n')
            .unwrap_or(&rendered_source),
    );

    let mut edited = source.to_string();
    edited.replace_range(
        selected_item.replacement_range.start..selected_item.replacement_range.end,
        &selected_item.label,
    );
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "EDITED");
    let rendered_edited = Source::new(&edited)
        .render()
        .expect("edited source should render");
    write_block(
        &mut snapshot,
        rendered_edited
            .strip_prefix('\n')
            .unwrap_or(&rendered_edited),
    );
    snapshot
}

fn hover_snapshot(source: &str, cursor: Position) -> String {
    let mut snapshot = String::new();
    let offset = cursor
        .to_byte_offset(source)
        .expect("cursor should be a valid source position");
    let output = hover(source, offset);
    write_section(&mut snapshot, "SOURCE");
    write_hover(&mut snapshot, source, cursor, output.as_ref());
    snapshot
}

fn write_hover(snapshot: &mut String, source: &str, cursor: Position, hover: Option<&Hover>) {
    let annotation = if let Some(hover) = hover {
        let range = Position::from_byte_offset(source, hover.range.start)
            .expect("hover start should be a valid source position")
            ..Position::from_byte_offset(source, hover.range.end)
                .expect("hover end should be a valid source position");
        Annotation::anchored_span(range, cursor, &hover.contents).boxed()
    } else {
        Annotation::point(cursor, "None")
    };
    let rendered = Source::new(source)
        .annotation(annotation)
        .render()
        .expect("hover should be a valid source annotation");
    write_block(snapshot, rendered.strip_prefix('\n').unwrap_or(&rendered));
}

fn signature_snapshot(source: &str, cursor: Position) -> String {
    let mut snapshot = String::new();
    let offset = cursor
        .to_byte_offset(source)
        .expect("cursor should be a valid source position");
    let output = signature_help(source, offset);
    write_section(&mut snapshot, "SOURCE");
    write_signature_help(&mut snapshot, source, cursor, output.as_ref());
    snapshot
}

fn write_signature_help(
    snapshot: &mut String,
    source: &str,
    cursor: Position,
    signature: Option<&SignatureHelp>,
) {
    let contents = signature.map(signature_help_contents);
    let annotation = if let Some(contents) = contents.as_deref() {
        Annotation::point(cursor, contents).boxed()
    } else {
        Annotation::point(cursor, "None")
    };
    let rendered = Source::new(source)
        .annotation(annotation)
        .render()
        .expect("signature help should be a valid source annotation");
    write_block(snapshot, rendered.strip_prefix('\n').unwrap_or(&rendered));
}

fn signature_help_contents(signature: &SignatureHelp) -> String {
    let mut contents = signature.signature.clone();
    if let Some(documentation) = &signature.documentation {
        write!(contents, "\n\n{documentation}").unwrap();
    }
    contents.push_str("\n\nparameters:\n\n");
    if signature.parameters.is_empty() {
        contents.push_str("None");
        return contents;
    }

    for (index, parameter) in signature.parameters.iter().enumerate() {
        if index > 0 {
            contents.push('\n');
        }
        let label = &signature.signature[parameter.label.start..parameter.label.end];
        write!(contents, "{}. ({label})", index + 1).unwrap();
        if let Some(documentation) = &parameter.documentation {
            write!(contents, " {documentation}").unwrap();
        }
        if signature.active_parameter == Some(index) {
            contents.push_str(" ←");
        }
    }
    contents
}

#[test]
fn completes_pipe_keywords() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | ",
        Position::new(0, 21),
        "map",
    ));
}

#[test]
fn completes_pipe_keywords_before_existing_code() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests |     rate",
        Position::new(0, 21),
        "map",
    ));
}

#[test]
fn completes_partial_pipe_keywords_before_existing_code() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | m    rate",
        Position::new(0, 22),
        "map",
    ));
}

#[test]
fn completes_align_functions() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | align $__interval using ",
        Position::new(0, 45),
        "prom::rate",
    ));
}

#[test]
fn completes_group_functions() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | group by host using ",
        Position::new(0, 41),
        "sum",
    ));
}

#[test]
fn completes_bucket_functions() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | bucket by le using ",
        Position::new(0, 40),
        "histogram",
    ));
}

#[test]
fn completes_compute_functions() {
    insta::assert_snapshot!(completions_snapshot(
        "compute total using ",
        Position::new(0, 20),
        "+",
    ));
}

#[test]
fn completes_map_functions_after_using() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | map using ",
        Position::new(0, 31),
        "fill::const",
    ));
}

#[test]
fn completes_map_functions_directly() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | map ",
        Position::new(0, 25),
        "rate",
    ));
}

#[test]
fn completes_source_start() {
    insta::assert_snapshot!(completions_snapshot("", Position::new(0, 0), "from"));
}

#[test]
fn completes_align_params() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | align ",
        Position::new(0, 27),
        "$__interval",
    ));
}

#[test]
fn completes_comparison_literals() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | where status == ",
        Position::new(0, 37),
        "true",
    ));
}

#[test]
fn completes_comparison_literals_with_following_lines() {
    insta::assert_snapshot!(completions_snapshot(
        "from prod:requests | where status == \n| map rate\n| as filtered_requests",
        Position::new(0, 37),
        "false",
    ));
}

#[test]
fn hovers_function() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | map fill::const(0)",
        Position::new(0, 31),
    ));
}

#[test]
fn hovers_keyword() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | align $__interval using avg",
        Position::new(0, 21),
    ));
}

#[test]
fn hovers_bare_map_function() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | map rate",
        Position::new(0, 27),
    ));
}

#[test]
fn hovers_namespaced_align_function() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | align $__interval using prom::rate",
        Position::new(0, 50),
    ));
}

#[test]
fn hovers_group_function_with_context_specific_docs() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | group by host using avg",
        Position::new(0, 42),
    ));
}

#[test]
fn hovers_bucket_function_with_arguments() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | bucket by le using histogram(\"le\")",
        Position::new(0, 44),
    ));
}

#[test]
fn hovers_compute_operator() {
    insta::assert_snapshot!(hover_snapshot(
        "compute total using +",
        Position::new(0, 20),
    ));
}

#[test]
fn hovers_pipe_token() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | map rate",
        Position::new(0, 19),
    ));
}

#[test]
fn hovers_source_keyword() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests\n| map rate",
        Position::new(0, 1),
    ));
}

#[test]
fn hovers_directive_keyword() {
    insta::assert_snapshot!(hover_snapshot(
        "set timezone = \"UTC\";\nfrom prod:requests",
        Position::new(0, 1),
    ));
}

#[test]
fn hovers_deprecated_filter_keyword() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | filter status == 500",
        Position::new(0, 23),
    ));
}

#[test]
fn hovers_alias_keyword() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | as requests",
        Position::new(0, 22),
    ));
}

#[test]
fn hovers_boolean_operator() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | where ok == true and ready == true",
        Position::new(0, 39),
    ));
}

#[test]
fn hovers_boolean_literal() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | where ok == true",
        Position::new(0, 34),
    ));
}

#[test]
fn does_not_hover_unknown_identifier() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests | map mystery",
        Position::new(0, 28),
    ));
}

#[test]
fn hovers_function_in_incomplete_multiline_pipeline() {
    insta::assert_snapshot!(hover_snapshot(
        "set timezone = \"UTC\";\nfrom prod:requests\n| where status ==\n| map fill::const(\n| align $__interval using",
        Position::new(3, 14),
    ));
}

#[test]
fn hovers_keyword_with_multiline_context_before_and_after() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests\n| where status ==\n| map rate\n| as requests",
        Position::new(1, 3),
    ));
}

#[test]
fn does_not_hover_partially_typed_function() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests\n| where status == true\n| map fill::con\n| as requests",
        Position::new(2, 13),
    ));
}

#[test]
fn does_not_hover_partially_typed_keyword() {
    insta::assert_snapshot!(hover_snapshot(
        "from prod:requests\n| whe\n| map rate",
        Position::new(1, 4),
    ));
}

#[test]
fn signature_help_for_function() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map fill::const(\n    )\n| as filled",
        Position::new(2, 4),
    ));
}

#[test]
fn signature_help_for_keyword() {
    insta::assert_snapshot!(signature_snapshot(
        "set timezone = \"UTC\";\nfrom prod:requests\n| group by host using sum\n| map rate",
        Position::new(2, 3),
    ));
}

#[test]
fn signature_help_for_group_tag() {
    insta::assert_snapshot!(signature_snapshot(
        "set timezone = \"UTC\";\nfrom prod:requests\n| group by host using sum\n| map rate",
        Position::new(2, 13),
    ));
}

#[test]
fn signature_help_after_group_by() {
    insta::assert_snapshot!(signature_snapshot(
        "set timezone = \"UTC\";\nfrom prod:requests\n| group by \n| map rate",
        Position::new(2, 11),
    ));
}

#[test]
fn signature_help_for_function_without_parameters() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map rate\n| where status == true",
        Position::new(1, 8),
    ));
}

#[test]
fn signature_help_inside_single_parameter() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map fill::const(\n    100\n)\n| as filled",
        Position::new(2, 5),
    ));
}

#[test]
fn signature_help_for_excess_non_variadic_argument() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map fill::const(\n    1,\n    2)\n| map rate",
        Position::new(3, 5),
    ));
}

#[test]
fn signature_help_for_first_variadic_argument() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using histogram(\n    0.5,\n    0.9,\n    0.99)\n| as histogrammed",
        Position::new(2, 5),
    ));
}

#[test]
fn signature_help_for_later_variadic_argument() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using histogram(\n    0.5,\n    0.9,\n    0.99)\n| as histogrammed",
        Position::new(4, 6),
    ));
}

#[test]
fn signature_help_for_first_multi_parameter_argument() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using interpolate_cumulative_histogram(\n    linear,\n    0.5,\n    0.9)\n| as interpolated",
        Position::new(2, 6),
    ));
}

#[test]
fn signature_help_for_variadic_multi_parameter_argument() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using interpolate_cumulative_histogram(\n    linear,\n    0.5,\n    0.9)\n| as interpolated",
        Position::new(4, 6),
    ));
}

#[test]
fn signature_help_after_comma_in_incomplete_variadic_call() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests | bucket by le using interpolate_cumulative_histogram(\n    linear,\n    0.5",
        Position::new(1, 11),
    ));
}

#[test]
fn signature_help_in_multiline_variadic_call() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using interpolate_cumulative_histogram(\n    linear,\n    0.5,\n    0.9\n)\n| as latency",
        Position::new(4, 5),
    ));
}

#[test]
fn no_signature_help_for_partially_typed_function() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map fill::con(1)\n| as requests",
        Position::new(1, 13),
    ));
}

#[test]
fn signature_help_immediately_after_opening_parenthesis() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map fill::const(\n| as later_code",
        Position::new(1, 18),
    ));
}

#[test]
fn signature_help_while_typing_single_parameter() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| map fill::const(10\n| as later_code",
        Position::new(1, 20),
    ));
}

#[test]
fn signature_help_while_typing_first_multi_parameter() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using interpolate_cumulative_histogram(\n    lin\n    0.5",
        Position::new(2, 7),
    ));
}

#[test]
fn signature_help_while_typing_variadic_parameter() {
    insta::assert_snapshot!(signature_snapshot(
        "from prod:requests\n| bucket by le using interpolate_cumulative_histogram(\n    linear,\n    0.\n    0.9",
        Position::new(3, 6),
    ));
}

#[test]
fn formats_from_syntax_tokens() {
    let input = "from prod:requests|align $__interval using avg;";
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}

#[test]
fn formats_parameters_and_set_directives() {
    let input = r#"// configuration


param   $duration
    :    Duration   ;

set     timezone    =    "UTC"    ;
"#;
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}

#[test]
fn formats_source_ranges_and_aliases() {
    let input = r#"

http  :   request_duration
       [   5m   ]

  as       latency

"#;
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}

#[test]
fn formats_predicates_and_type_checks() {
    let input = r#"from    prod : requests
             |    where     service
       ==       "api"

 and       not
          status    is       string
"#;
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}

#[test]
fn formats_function_calls_and_map_pipelines() {
    let input = r#"from prod:requests
  |       map
              fill  ::  const (
       0
                            )



         |map       rate
"#;
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}

#[test]
fn formats_group_bucket_and_extend_pipeline() {
    let input = r#"from prod:requests
       |group    by service,
 region     using       sum

 |       bucket by le
       to       1m using
 histogram ( "le" )


            |extend    slo =99.9,
matcher=   #/api/
"#;
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}

#[test]
fn formats_parenthesized_compute_queries() {
    let input = r#"(
       cpu : usage [ 1h ]
 |map       rate,

mem : rss [1h]
                )
        | compute
 total       using avg


 |as        total_usage
"#;
    insta::assert_snapshot!(format_snapshot(input, &format(input)));
}
