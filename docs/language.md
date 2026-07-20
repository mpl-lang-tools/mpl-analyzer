# MPL Language Contract

This document describes the analyzer's current compatibility boundary. Public
MPL documentation and the canonical
[`axiomhq/mpl`](https://github.com/axiomhq/mpl) repository, including verified
local checkouts of it, are behavioral references. The analyzer remains an
independent implementation and does not call `mplc`.

The canonical `mpl/tests/examples` corpus is expected to produce no error-level
analyzer diagnostics, and every fixture in `mpl/tests/errors` is expected to
produce at least one error. Valid queries may still produce warnings or hints
for accepted but deprecated or non-canonical syntax. This is a regression
baseline, not a claim of complete compiler equivalence.

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

## Warnings and hints

After a query parses and passes semantic validation, the analyzer reports the
same source-level warnings and hints exposed by MPL's legacy language server:

- lowercase `duration` parameter types produce a warning and a replacement edit
  for `Duration`;
- user parameter declarations beginning with the reserved `$__` prefix produce
  a warning;
- deprecated `filter` operations produce a hint and a replacement edit for
  `where`; and
- backtick-escaped identifiers that are valid plain identifiers produce a hint
  and an edit that removes the backticks.

Warnings emitted while validating externally supplied runtime parameter values,
such as a provided parameter that was not declared in the MPL source, are not
part of source analysis because neither the IDE API nor LSP receives those
runtime values.

## Partial or deferred coverage

- `join` and `replace` are structurally parsed, but do not yet have dedicated
  HIR nodes or semantic validation.
- `ifdef` and `sample` are recognized for lossless parsing and recovery; their
  bodies are not semantically modeled.
- Parameter declarations are recognized and used for name validation and
  source-level warnings, but their types and defaults are not lowered into the
  HIR model.
- Time-range contents are preserved as source text rather than semantically
  interpreted.
- Function validation is catalog-based; general arity and type checking remain
  incomplete.
- Generic directive semantics, WASM functions, and compiler/query-engine
  execution are not implemented.
