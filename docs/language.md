# MPL Language Contract

This document describes the analyzer's current compatibility boundary. Public
MPL documentation and the sibling MPL implementation are behavioral references;
the analyzer remains an independent implementation and does not call `mplc`.

The canonical `mpl/tests/examples` corpus currently produces no analyzer
diagnostics, and every fixture in `mpl/tests/errors` produces at least one. This
is a regression baseline, not a claim of complete compiler equivalence.

## Syntax coverage

- Prelude declarations: `set` directives and user `param` declarations.
- Sources: `dataset:metric`, parameters in source names, optional bracketed time
  range, and optional `as` alias.
- Queries: simple pipelines and parenthesized multi-input queries followed by a
  `compute ... using ...` rule.
- Filters: `where`, deprecated `filter`, comparisons, type checks, `and`, `or`,
  `not`, and parenthesized predicates.
- Transformations: `map`, `align` (including `to` and `over`), `group`,
  `bucket`, `extend`, `as`, `join`, `replace`, `ifdef`, and `sample`.
- Functions: bare, namespaced, and call forms with comma-separated arguments.
- Literals: strings, integers, floats, booleans, regexes, substitution regexes,
  durations, timestamps, and parameter references.
- Identifiers: plain identifiers, dotted or `::`-qualified names, and
  backtick-escaped identifiers.
- Trivia: whitespace and `//` line comments are retained losslessly.

## Semantic checks

HIR validation currently checks:

- missing sources and missing compute rules;
- required `using` functions for align, group, bucket, and compute operations;
- function names against the map, align, group, bucket, and compute catalogs;
- declared parameters and the built-in `$__interval`, including parameters used
  as dataset or metric names;
- histogram bucket specifications and the `rate`/`increase` conversion required
  by cumulative histograms; and
- syntax failures such as unknown pipeline operations and unsupported duration
  suffixes.

## Partial or deferred coverage

- `join` and `replace` are structurally parsed, but do not yet have dedicated
  HIR nodes or semantic validation.
- `ifdef` and `sample` are recognized for lossless parsing and recovery; their
  bodies are not semantically modeled.
- Parameter declarations are recognized and used for name validation, but their
  types and defaults are not lowered into the HIR model.
- Time-range contents are preserved as source text rather than semantically
  interpreted.
- Function validation is catalog-based; general arity and type checking remain
  incomplete.
- Generic directive semantics, WASM functions, and compiler/query-engine
  execution are not implemented.
