//! Lowering from typed CST wrappers into HIR.
//!
//! This module is the boundary between concrete syntax and semantic structure.
//! It copies the source ranges needed by later diagnostics and editor features
//! while discarding trivia and CST details that should not leak into HIR.

use mpl_syntax::{
    AssignmentNode, AstNode, ComputeRuleNode, DirectiveNode, ExprKind, ExprNode, FunctionCallNode,
    NameRefNode, Parse, PipeKind, PipeNode, QueryNode, SourceFileNode, SourceNode, SyntaxElement,
    SyntaxKind, SyntaxNode, SyntaxToken, ast_children, token_range,
};

use crate::model::{
    AlignPipe, AsPipe, Assignment, BucketPipe, ComputeQuery, ComputeRule, Directive, Expr,
    ExtendPipe, FunctionCall, FunctionPipe, GroupPipe, HirFile, NameRef, Pipe, Query, SimpleQuery,
    Source, TimeRange, UnknownPipe, WherePipe,
};

pub fn lower(parse: &Parse<SourceFileNode>) -> HirFile {
    lower_file(parse.tree())
}

fn lower_file(file: &SourceFileNode) -> HirFile {
    HirFile {
        directives: file
            .directives()
            .map(|directive| lower_directive(&directive))
            .collect(),
        query: file.queries().next().map(|query| lower_query(&query)),
        range: file.range(),
    }
}

fn lower_directive(directive: &DirectiveNode) -> Directive {
    Directive {
        name: directive.name().map(|name| lower_name(&name)),
        value: directive.value().map(|expr| lower_expr(&expr)),
        range: directive.range(),
    }
}

fn lower_query(query: &QueryNode) -> Query {
    match query.syntax().kind() {
        SyntaxKind::ComputeQuery => Query::Compute(ComputeQuery {
            inputs: query
                .input_queries()
                .map(|input| lower_query(&input))
                .collect(),
            rule: query.compute_rule().map(|rule| lower_compute_rule(&rule)),
            pipes: query.pipes().map(|pipe| lower_pipe(&pipe)).collect(),
            range: query.range(),
        }),
        _ => Query::Simple(SimpleQuery {
            source: query.sources().next().map(|source| lower_source(&source)),
            pipes: query.pipes().map(|pipe| lower_pipe(&pipe)).collect(),
            range: query.range(),
        }),
    }
}

fn lower_source(source: &SourceNode) -> Source {
    Source {
        dataset: source.dataset().map(|name| lower_name(&name)),
        metric: source.metric().map(|name| lower_name(&name)),
        time_range: source.time_range().map(|range| TimeRange {
            text: range.text(),
            range: range.range(),
        }),
        alias: source.alias().map(|name| lower_name(&name)),
        range: source.range(),
    }
}

fn lower_compute_rule(rule: &ComputeRuleNode) -> ComputeRule {
    ComputeRule {
        name: rule.name().map(|name| lower_name(&name)),
        function: rule.function().map(|function| lower_function(&function)),
        range: rule.range(),
    }
}

fn lower_pipe(pipe: &PipeNode) -> Pipe {
    match pipe.kind() {
        PipeKind::Where => Pipe::Where(WherePipe {
            keyword: pipe.keyword_text(),
            predicates: pipe.exprs().map(|expr| lower_expr(&expr)).collect(),
            range: pipe.range(),
        }),
        PipeKind::Map => Pipe::Map(FunctionPipe {
            function: pipe.function().map(|function| lower_function(&function)),
            exprs: pipe.exprs().map(|expr| lower_expr(&expr)).collect(),
            range: pipe.range(),
        }),
        PipeKind::Align => Pipe::Align(AlignPipe {
            window: pipe.exprs().next().map(|expr| lower_expr(&expr)),
            function: pipe.function().map(|function| lower_function(&function)),
            range: pipe.range(),
        }),
        PipeKind::Group => Pipe::Group(GroupPipe {
            tags: direct_names(pipe.syntax())
                .map(|name| lower_name(&name))
                .collect(),
            function: pipe.function().map(|function| lower_function(&function)),
            range: pipe.range(),
        }),
        PipeKind::Bucket => Pipe::Bucket(BucketPipe {
            tags: direct_names(pipe.syntax())
                .map(|name| lower_name(&name))
                .collect(),
            window: pipe.exprs().next().map(|expr| lower_expr(&expr)),
            function: pipe.function().map(|function| lower_function(&function)),
            range: pipe.range(),
        }),
        PipeKind::Extend => Pipe::Extend(ExtendPipe {
            assignments: pipe
                .assignments()
                .map(|assignment| lower_assignment(&assignment))
                .collect(),
            range: pipe.range(),
        }),
        PipeKind::As => Pipe::As(AsPipe {
            alias: direct_names(pipe.syntax())
                .next()
                .map(|name| lower_name(&name)),
            range: pipe.range(),
        }),
        PipeKind::Unknown => Pipe::Unknown(UnknownPipe {
            keyword: pipe.keyword_text(),
            range: pipe.range(),
        }),
    }
}

fn lower_assignment(assignment: &AssignmentNode) -> Assignment {
    Assignment {
        name: assignment.name().map(|name| lower_name(&name)),
        value: assignment.value().map(|expr| lower_expr(&expr)),
        range: assignment.range(),
    }
}

fn lower_expr(expr: &ExprNode) -> Expr {
    match expr.kind() {
        ExprKind::String => Expr::String {
            text: expr.text(),
            range: expr.range(),
        },
        ExprKind::Number => Expr::Number {
            text: expr.text(),
            range: expr.range(),
        },
        ExprKind::Duration => Expr::Duration {
            text: expr.text(),
            range: expr.range(),
        },
        ExprKind::Timestamp => Expr::Timestamp {
            text: expr.text(),
            range: expr.range(),
        },
        ExprKind::Bool => Expr::Bool {
            text: expr.text(),
            range: expr.range(),
        },
        ExprKind::Regex => Expr::Regex {
            text: expr.text(),
            range: expr.range(),
        },
        ExprKind::Param => Expr::Param {
            name: token_name(expr.token(), expr.range()),
            range: expr.range(),
        },
        ExprKind::Name => Expr::Name {
            name: expr
                .name()
                .map(|name| lower_name(&name))
                .unwrap_or_else(|| missing_name(expr.range())),
            range: expr.range(),
        },
        ExprKind::Call => lower_call_expr(expr),
        ExprKind::Missing => Expr::Missing {
            range: expr.range(),
        },
        ExprKind::Not => Expr::Not {
            expr: expr.exprs().next().map(|expr| Box::new(lower_expr(&expr))),
            range: expr.range(),
        },
        ExprKind::Compare => Expr::Compare {
            lhs: direct_names(expr.syntax())
                .next()
                .map(|name| lower_name(&name)),
            op: comparison_operator(expr.syntax()),
            rhs: expr.exprs().next().map(|expr| Box::new(lower_expr(&expr))),
            range: expr.range(),
        },
        ExprKind::TypeCheck => {
            let mut names = direct_names(expr.syntax());
            Expr::TypeCheck {
                lhs: names.next().map(|name| lower_name(&name)),
                ty: names.next().map(|name| lower_name(&name)),
                range: expr.range(),
            }
        }
        ExprKind::Paren => Expr::Paren {
            expr: expr.exprs().next().map(|expr| Box::new(lower_expr(&expr))),
            range: expr.range(),
        },
        ExprKind::Binary => Expr::Missing {
            range: expr.range(),
        },
    }
}

fn lower_call_expr(expr: &ExprNode) -> Expr {
    let Some(call) = expr
        .function_call()
        .map(|function| lower_function(&function))
    else {
        return Expr::Missing {
            range: expr.range(),
        };
    };

    if !call.has_arg_list
        && call.args.is_empty()
        && !call.name.text.contains("::")
        && !call.name.text.contains('.')
        && !is_operator_name(&call.name.text)
    {
        Expr::Name {
            name: call.name,
            range: expr.range(),
        }
    } else {
        Expr::Call {
            call,
            range: expr.range(),
        }
    }
}

fn lower_function(function: &FunctionCallNode) -> FunctionCall {
    FunctionCall {
        name: function
            .name()
            .map(|name| lower_name(&name))
            .or_else(|| {
                function
                    .operator()
                    .map(|token| token_name(Some(token), function.range()))
            })
            .unwrap_or_else(|| missing_name(function.range())),
        args: function.args().map(|arg| lower_expr(&arg)).collect(),
        has_arg_list: function.has_arg_list(),
        range: function.range(),
    }
}

fn lower_name(name: &NameRefNode) -> NameRef {
    NameRef {
        text: name.text(),
        range: name.range(),
    }
}

fn direct_names(node: &SyntaxNode) -> impl Iterator<Item = NameRefNode> + '_ {
    ast_children::<NameRefNode>(node)
}

fn comparison_operator(node: &SyntaxNode) -> Option<String> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|token| matches!(token.kind(), SyntaxKind::Cmp | SyntaxKind::Eq))
        .map(|token| token.text().to_owned())
}

fn token_name(token: Option<SyntaxToken>, fallback_range: mpl_syntax::TextRange) -> NameRef {
    token
        .map(|token| NameRef {
            text: token.text().to_owned(),
            range: token_range(&token),
        })
        .unwrap_or_else(|| missing_name(fallback_range))
}

fn missing_name(range: mpl_syntax::TextRange) -> NameRef {
    NameRef {
        text: String::new(),
        range,
    }
}

fn is_operator_name(text: &str) -> bool {
    matches!(text, "+" | "-" | "*" | "/")
}
