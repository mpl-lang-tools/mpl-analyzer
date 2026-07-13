//! Snapshot coverage for HIR lowering.
//!
//! These tests lock down the semantic structure produced from representative
//! MPL inputs. Snapshots include both the source fixture and a tree-shaped HIR
//! rendering so source mapping and semantic shape changes are reviewable
//! together.

use display_tree::{AsTree, CharSet, DisplayTree, Style, StyleBuilder};
use mpl_hir::{
    AlignPipe, AsPipe, Assignment, BucketPipe, ComputeQuery, ComputeRule, Directive, Expr,
    FunctionCall, FunctionPipe, GroupPipe, HirFile, NameRef, Pipe, Query, SimpleQuery, Source,
    TimeRange, UnknownPipe, WherePipe, lower,
};
use mpl_syntax::parse_syntax;
use std::fmt::{self, Write};

struct HirTree {
    label: String,
    children: Vec<HirTree>,
}

impl DisplayTree for HirTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, style: Style) -> fmt::Result {
        writeln!(f, "{}", style.leaf_style.apply(&self.label))?;
        self.fmt_children(f, style, "")
    }
}

impl HirTree {
    fn leaf(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            children: Vec::new(),
        }
    }

    fn node(label: impl Into<String>, children: Vec<HirTree>) -> Self {
        Self {
            label: label.into(),
            children,
        }
    }

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

fn display_hir(hir: &HirFile) -> String {
    AsTree::new(&hir_file(hir))
        .indentation(2)
        .char_set(CharSet::SINGLE_LINE_CURVED)
        .to_string()
}

fn snapshot(input: &str) -> String {
    let parse = parse_syntax(input);
    let hir = lower(&parse);
    let mut snapshot = String::new();
    write_section(&mut snapshot, "INPUT");
    write_block(&mut snapshot, input);
    writeln!(snapshot).unwrap();
    write_section(&mut snapshot, "HIR");
    write_block(&mut snapshot, &display_hir(&hir));
    snapshot
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

fn hir_file(file: &HirFile) -> HirTree {
    HirTree::node(
        "HirFile",
        vec![
            HirTree::node(
                "directives",
                file.directives.iter().map(directive).collect(),
            ),
            optional_query("query", file.query.as_ref()),
        ],
    )
}

fn directive(directive: &Directive) -> HirTree {
    HirTree::node(
        "Directive",
        vec![
            optional_name("name", directive.name.as_ref()),
            optional_expr("value", directive.value.as_ref()),
        ],
    )
}

fn query_tree(query: &Query) -> HirTree {
    match query {
        Query::Simple(query) => simple_query(query),
        Query::Compute(query) => compute_query(query),
    }
}

fn simple_query(query: &SimpleQuery) -> HirTree {
    HirTree::node(
        "SimpleQuery",
        vec![
            optional_source("source", query.source.as_ref()),
            list("pipes", query.pipes.iter().map(pipe).collect()),
        ],
    )
}

fn compute_query(query: &ComputeQuery) -> HirTree {
    HirTree::node(
        "ComputeQuery",
        vec![
            list("inputs", query.inputs.iter().map(query_tree).collect()),
            optional_compute_rule("rule", query.rule.as_ref()),
            list("pipes", query.pipes.iter().map(pipe).collect()),
        ],
    )
}

fn source(source: &Source) -> HirTree {
    HirTree::node(
        "Source",
        vec![
            optional_name("dataset", source.dataset.as_ref()),
            optional_name("metric", source.metric.as_ref()),
            optional_time_range("time_range", source.time_range.as_ref()),
            optional_name("alias", source.alias.as_ref()),
        ],
    )
}

fn time_range(time_range: &TimeRange) -> HirTree {
    HirTree::leaf(format!("TimeRange {}", quoted(&time_range.text)))
}

fn pipe(pipe: &Pipe) -> HirTree {
    match pipe {
        Pipe::Where(pipe) => where_pipe(pipe),
        Pipe::Map(pipe) => function_pipe("MapPipe", pipe),
        Pipe::Align(pipe) => align_pipe(pipe),
        Pipe::Group(pipe) => group_pipe(pipe),
        Pipe::Bucket(pipe) => bucket_pipe(pipe),
        Pipe::Extend(pipe) => extend_pipe(pipe),
        Pipe::As(pipe) => as_pipe(pipe),
        Pipe::Unknown(pipe) => unknown_pipe(pipe),
    }
}

fn where_pipe(pipe: &WherePipe) -> HirTree {
    HirTree::node(
        format!("WherePipe keyword={}", optional_text(&pipe.keyword)),
        vec![list(
            "predicates",
            pipe.predicates.iter().map(expr).collect(),
        )],
    )
}

fn function_pipe(label: &str, pipe: &FunctionPipe) -> HirTree {
    HirTree::node(
        label,
        vec![
            optional_function("function", pipe.function.as_ref()),
            list("exprs", pipe.exprs.iter().map(expr).collect()),
        ],
    )
}

fn align_pipe(pipe: &AlignPipe) -> HirTree {
    HirTree::node(
        "AlignPipe",
        vec![
            optional_expr("window", pipe.window.as_ref()),
            optional_function("function", pipe.function.as_ref()),
        ],
    )
}

fn group_pipe(pipe: &GroupPipe) -> HirTree {
    HirTree::node(
        "GroupPipe",
        vec![
            list("tags", pipe.tags.iter().map(name).collect()),
            optional_function("function", pipe.function.as_ref()),
        ],
    )
}

fn bucket_pipe(pipe: &BucketPipe) -> HirTree {
    HirTree::node(
        "BucketPipe",
        vec![
            list("tags", pipe.tags.iter().map(name).collect()),
            optional_expr("window", pipe.window.as_ref()),
            optional_function("function", pipe.function.as_ref()),
        ],
    )
}

fn extend_pipe(pipe: &mpl_hir::ExtendPipe) -> HirTree {
    HirTree::node(
        "ExtendPipe",
        vec![list(
            "assignments",
            pipe.assignments.iter().map(assignment).collect(),
        )],
    )
}

fn as_pipe(pipe: &AsPipe) -> HirTree {
    HirTree::node("AsPipe", vec![optional_name("alias", pipe.alias.as_ref())])
}

fn unknown_pipe(pipe: &UnknownPipe) -> HirTree {
    HirTree::leaf(format!(
        "UnknownPipe keyword={}",
        optional_text(&pipe.keyword)
    ))
}

fn compute_rule(rule: &ComputeRule) -> HirTree {
    HirTree::node(
        "ComputeRule",
        vec![
            optional_name("name", rule.name.as_ref()),
            optional_function("function", rule.function.as_ref()),
        ],
    )
}

fn assignment(assignment: &Assignment) -> HirTree {
    HirTree::node(
        "Assignment",
        vec![
            optional_name("name", assignment.name.as_ref()),
            optional_expr("value", assignment.value.as_ref()),
        ],
    )
}

fn function(function: &FunctionCall) -> HirTree {
    HirTree::node(
        format!("FunctionCall has_arg_list={}", function.has_arg_list),
        vec![
            HirTree::node("name", vec![name(&function.name)]),
            list("args", function.args.iter().map(expr).collect()),
        ],
    )
}

fn expr(expr: &Expr) -> HirTree {
    match expr {
        Expr::String { text, .. } => literal("String", text),
        Expr::Number { text, .. } => literal("Number", text),
        Expr::Duration { text, .. } => literal("Duration", text),
        Expr::Timestamp { text, .. } => literal("Timestamp", text),
        Expr::Bool { text, .. } => literal("Bool", text),
        Expr::Regex { text, .. } => literal("Regex", text),
        Expr::Param { name, .. } => {
            HirTree::node("Param", vec![HirTree::node("name", vec![self::name(name)])])
        }
        Expr::Name { name, .. } => HirTree::node(
            "NameExpr",
            vec![HirTree::node("name", vec![self::name(name)])],
        ),
        Expr::Call { call, .. } => HirTree::node("CallExpr", vec![function(call)]),
        Expr::Missing { .. } => HirTree::leaf("Missing"),
        Expr::Not { expr, .. } => {
            HirTree::node("Not", vec![optional_expr("expr", expr.as_deref())])
        }
        Expr::Compare { lhs, op, rhs, .. } => HirTree::node(
            format!("Compare op={}", optional_text(op)),
            vec![
                optional_name("lhs", lhs.as_ref()),
                optional_expr("rhs", rhs.as_deref()),
            ],
        ),
        Expr::TypeCheck { lhs, ty, .. } => HirTree::node(
            "TypeCheck",
            vec![
                optional_name("lhs", lhs.as_ref()),
                optional_name("ty", ty.as_ref()),
            ],
        ),
        Expr::Paren { expr, .. } => {
            HirTree::node("Paren", vec![optional_expr("expr", expr.as_deref())])
        }
    }
}

fn literal(kind: &str, text: &str) -> HirTree {
    HirTree::leaf(format!("{kind} {}", quoted(text)))
}

fn name(name: &NameRef) -> HirTree {
    HirTree::leaf(format!("NameRef {}", quoted(&name.text)))
}

fn list(label: &str, children: Vec<HirTree>) -> HirTree {
    HirTree::node(label, children)
}

fn optional_query(label: &str, value: Option<&Query>) -> HirTree {
    optional(label, value.map(query_tree))
}

fn optional_source(label: &str, value: Option<&Source>) -> HirTree {
    optional(label, value.map(source))
}

fn optional_time_range(label: &str, value: Option<&TimeRange>) -> HirTree {
    optional(label, value.map(time_range))
}

fn optional_compute_rule(label: &str, value: Option<&ComputeRule>) -> HirTree {
    optional(label, value.map(compute_rule))
}

fn optional_name(label: &str, value: Option<&NameRef>) -> HirTree {
    optional(label, value.map(name))
}

fn optional_function(label: &str, value: Option<&FunctionCall>) -> HirTree {
    optional(label, value.map(function))
}

fn optional_expr(label: &str, value: Option<&Expr>) -> HirTree {
    optional(label, value.map(expr))
}

fn optional(label: &str, value: Option<HirTree>) -> HirTree {
    match value {
        Some(value) => HirTree::node(label, vec![value]),
        None => HirTree::leaf(format!("{label}: None")),
    }
}

fn optional_text(value: &Option<String>) -> String {
    value
        .as_deref()
        .map(quoted)
        .unwrap_or_else(|| "None".into())
}

fn quoted(text: &str) -> String {
    format!("{text:?}")
}

#[test]
fn lowers_public_query_shape() {
    insta::assert_snapshot!(snapshot(
        r#"set custom_unit = "ms";
http:request_duration[5m] as latency
| where service == "api" and not status is string
| map fill::const(0)
| align to $__interval using avg
| group by service, region using sum
| bucket by status to 1m using histogram("le")
| extend slo = 99.9, matcher = #/api\/v1/
| as api_latency
"#,
    ));
}

#[test]
fn lowers_compute_query_shape() {
    insta::assert_snapshot!(snapshot(
        r#"(cpu:usage[1h] | map rate, mem:rss[1h]) | compute total using avg | as total_usage"#,
    ));
}
