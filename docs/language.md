# MPL Language Contract

This document tracks the analyzer's current MPL compatibility target. The v1
target is the public Axiom MPL documentation, not every construct observed in
the local MPL implementation.

## V1

- Source expressions: `dataset:metric`, optional time range, optional source
  alias.
- Directives: `set custom_unit = "..."`.
- Filters: `where`, comparisons, type checks, `and`, `or`, `not`, grouping.
- Transformations: `map`, `align`, `group`, `bucket`, `extend`, `compute`.
- Literals: strings, integers, floats, booleans, regexes, durations, timestamps.
- Identifiers: plain ASCII identifiers and backtick-escaped identifiers.
- Built-in host parameter: `$__interval`.
- Comments: line comments beginning with `//`.

## Deferred

- User `param` declarations.
- `ifdef`.
- `sample`.
- `replace`.
- `join`.
- Sliding windows with `over`.
- Generic directives beyond `custom_unit`.
- WASM functions.

