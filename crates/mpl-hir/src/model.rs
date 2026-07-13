//! HIR data model for MPL analysis.
//!
//! These types describe the semantic shape that validation and future IDE
//! features consume: queries, sources, pipes, expressions, functions, and source
//! ranges. The model is intentionally serializable so snapshot tests can review
//! lowered structure directly.

use serde::Serialize;

use mpl_syntax::TextRange;

#[derive(Debug, Clone, Serialize)]
pub struct HirFile {
    pub directives: Vec<Directive>,
    pub query: Option<Query>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct Directive {
    pub name: Option<NameRef>,
    pub value: Option<Expr>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Query {
    Simple(SimpleQuery),
    Compute(ComputeQuery),
}

impl Query {
    pub fn range(&self) -> TextRange {
        match self {
            Query::Simple(query) => query.range,
            Query::Compute(query) => query.range,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SimpleQuery {
    pub source: Option<Source>,
    pub pipes: Vec<Pipe>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputeQuery {
    pub inputs: Vec<Query>,
    pub rule: Option<ComputeRule>,
    pub pipes: Vec<Pipe>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub dataset: Option<NameRef>,
    pub metric: Option<NameRef>,
    pub time_range: Option<TimeRange>,
    pub alias: Option<NameRef>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeRange {
    pub text: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pipe {
    Where(WherePipe),
    Map(FunctionPipe),
    Align(AlignPipe),
    Group(GroupPipe),
    Bucket(BucketPipe),
    Extend(ExtendPipe),
    As(AsPipe),
    Unknown(UnknownPipe),
}

impl Pipe {
    pub fn range(&self) -> TextRange {
        match self {
            Pipe::Where(pipe) => pipe.range,
            Pipe::Map(pipe) => pipe.range,
            Pipe::Align(pipe) => pipe.range,
            Pipe::Group(pipe) => pipe.range,
            Pipe::Bucket(pipe) => pipe.range,
            Pipe::Extend(pipe) => pipe.range,
            Pipe::As(pipe) => pipe.range,
            Pipe::Unknown(pipe) => pipe.range,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WherePipe {
    pub keyword: Option<String>,
    pub predicates: Vec<Expr>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionPipe {
    pub function: Option<FunctionCall>,
    pub exprs: Vec<Expr>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlignPipe {
    pub window: Option<Expr>,
    pub function: Option<FunctionCall>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupPipe {
    pub tags: Vec<NameRef>,
    pub function: Option<FunctionCall>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct BucketPipe {
    pub tags: Vec<NameRef>,
    pub window: Option<Expr>,
    pub function: Option<FunctionCall>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtendPipe {
    pub assignments: Vec<Assignment>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct AsPipe {
    pub alias: Option<NameRef>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnknownPipe {
    pub keyword: Option<String>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputeRule {
    pub name: Option<NameRef>,
    pub function: Option<FunctionCall>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct Assignment {
    pub name: Option<NameRef>,
    pub value: Option<Expr>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionCall {
    pub name: NameRef,
    pub args: Vec<Expr>,
    pub has_arg_list: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expr {
    String {
        text: String,
        range: TextRange,
    },
    Number {
        text: String,
        range: TextRange,
    },
    Duration {
        text: String,
        range: TextRange,
    },
    Timestamp {
        text: String,
        range: TextRange,
    },
    Bool {
        text: String,
        range: TextRange,
    },
    Regex {
        text: String,
        range: TextRange,
    },
    Param {
        name: NameRef,
        range: TextRange,
    },
    Name {
        name: NameRef,
        range: TextRange,
    },
    Call {
        call: FunctionCall,
        range: TextRange,
    },
    Missing {
        range: TextRange,
    },
    Not {
        expr: Option<Box<Expr>>,
        range: TextRange,
    },
    Compare {
        lhs: Option<NameRef>,
        op: Option<String>,
        rhs: Option<Box<Expr>>,
        range: TextRange,
    },
    TypeCheck {
        lhs: Option<NameRef>,
        ty: Option<NameRef>,
        range: TextRange,
    },
    Paren {
        expr: Option<Box<Expr>>,
        range: TextRange,
    },
}

impl Expr {
    pub fn range(&self) -> TextRange {
        match self {
            Expr::String { range, .. }
            | Expr::Number { range, .. }
            | Expr::Duration { range, .. }
            | Expr::Timestamp { range, .. }
            | Expr::Bool { range, .. }
            | Expr::Regex { range, .. }
            | Expr::Param { range, .. }
            | Expr::Name { range, .. }
            | Expr::Call { range, .. }
            | Expr::Missing { range }
            | Expr::Not { range, .. }
            | Expr::Compare { range, .. }
            | Expr::TypeCheck { range, .. }
            | Expr::Paren { range, .. } => *range,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NameRef {
    pub text: String,
    pub range: TextRange,
}
