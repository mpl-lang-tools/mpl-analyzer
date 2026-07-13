//! Built-in MPL function and keyword catalog.
//!
//! This module centralizes the clean-room knowledge used by semantic
//! validation, completions, hover, and signature help. It is deliberately small
//! and structured so richer documentation and function metadata can replace it
//! without changing caller APIs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    Map,
    Align,
    Group,
    Bucket,
    Compute,
}

impl FunctionKind {
    pub const fn name(self) -> &'static str {
        match self {
            FunctionKind::Map => "map",
            FunctionKind::Align => "align",
            FunctionKind::Group => "group",
            FunctionKind::Bucket => "bucket",
            FunctionKind::Compute => "compute",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Function {
    pub name: &'static str,
    pub kind: FunctionKind,
    pub signature: &'static str,
    pub docs: &'static str,
}

pub const FUNCTIONS: &[Function] = &[
    Function {
        name: "rate",
        kind: FunctionKind::Map,
        signature: "rate",
        docs: "Per-second rate of change.",
    },
    Function {
        name: "increase",
        kind: FunctionKind::Map,
        signature: "increase",
        docs: "Increase from the previous datapoint.",
    },
    Function {
        name: "abs",
        kind: FunctionKind::Map,
        signature: "abs",
        docs: "Absolute value.",
    },
    Function {
        name: "fill::prev",
        kind: FunctionKind::Map,
        signature: "fill::prev",
        docs: "Fill missing values from the previous datapoint.",
    },
    Function {
        name: "fill::const",
        kind: FunctionKind::Map,
        signature: "fill::const(value)",
        docs: "Fill missing values with a constant.",
    },
    Function {
        name: "filter::lt",
        kind: FunctionKind::Map,
        signature: "filter::lt(value)",
        docs: "Keep datapoints less than a value.",
    },
    Function {
        name: "is::lt",
        kind: FunctionKind::Map,
        signature: "is::lt(value)",
        docs: "Emit 1.0 when a datapoint is less than a value, otherwise 0.0.",
    },
    Function {
        name: "avg",
        kind: FunctionKind::Align,
        signature: "avg",
        docs: "Average values.",
    },
    Function {
        name: "count",
        kind: FunctionKind::Align,
        signature: "count",
        docs: "Count values.",
    },
    Function {
        name: "last",
        kind: FunctionKind::Align,
        signature: "last",
        docs: "Last value.",
    },
    Function {
        name: "max",
        kind: FunctionKind::Align,
        signature: "max",
        docs: "Maximum value.",
    },
    Function {
        name: "min",
        kind: FunctionKind::Align,
        signature: "min",
        docs: "Minimum value.",
    },
    Function {
        name: "sum",
        kind: FunctionKind::Align,
        signature: "sum",
        docs: "Sum values.",
    },
    Function {
        name: "prom::rate",
        kind: FunctionKind::Align,
        signature: "prom::rate",
        docs: "PromQL-style rate calculation.",
    },
    Function {
        name: "avg",
        kind: FunctionKind::Group,
        signature: "avg",
        docs: "Average grouped series.",
    },
    Function {
        name: "count",
        kind: FunctionKind::Group,
        signature: "count",
        docs: "Count grouped values.",
    },
    Function {
        name: "max",
        kind: FunctionKind::Group,
        signature: "max",
        docs: "Maximum grouped value.",
    },
    Function {
        name: "min",
        kind: FunctionKind::Group,
        signature: "min",
        docs: "Minimum grouped value.",
    },
    Function {
        name: "sum",
        kind: FunctionKind::Group,
        signature: "sum",
        docs: "Sum grouped values.",
    },
    Function {
        name: "histogram",
        kind: FunctionKind::Bucket,
        signature: "histogram(specs...)",
        docs: "Aggregate non-histogram series into buckets.",
    },
    Function {
        name: "interpolate_cumulative_histogram",
        kind: FunctionKind::Bucket,
        signature: "interpolate_cumulative_histogram(mode, specs...)",
        docs: "Aggregate cumulative histogram series.",
    },
    Function {
        name: "interpolate_delta_histogram",
        kind: FunctionKind::Bucket,
        signature: "interpolate_delta_histogram(specs...)",
        docs: "Aggregate delta histogram series.",
    },
    Function {
        name: "+",
        kind: FunctionKind::Compute,
        signature: "+",
        docs: "Add results.",
    },
    Function {
        name: "-",
        kind: FunctionKind::Compute,
        signature: "-",
        docs: "Subtract results.",
    },
    Function {
        name: "*",
        kind: FunctionKind::Compute,
        signature: "*",
        docs: "Multiply results.",
    },
    Function {
        name: "/",
        kind: FunctionKind::Compute,
        signature: "/",
        docs: "Divide results.",
    },
    Function {
        name: "avg",
        kind: FunctionKind::Compute,
        signature: "avg",
        docs: "Average results.",
    },
    Function {
        name: "max",
        kind: FunctionKind::Compute,
        signature: "max",
        docs: "Maximum result.",
    },
    Function {
        name: "min",
        kind: FunctionKind::Compute,
        signature: "min",
        docs: "Minimum result.",
    },
];

pub const BUILTIN_PARAMS: &[&str] = &["$__interval"];

pub fn functions_by_kind(kind: FunctionKind) -> impl Iterator<Item = &'static Function> {
    FUNCTIONS
        .iter()
        .filter(move |function| function.kind == kind)
}

pub fn find_function(kind: FunctionKind, name: &str) -> Option<&'static Function> {
    functions_by_kind(kind).find(|function| function.name == name)
}

pub fn is_function(kind: FunctionKind, name: &str) -> bool {
    find_function(kind, name).is_some()
}

pub fn is_builtin_param(name: &str) -> bool {
    BUILTIN_PARAMS.contains(&name)
}
