# Roadmap

Major work left, roughly in dependency order:

1. **Automated mplc compatibility checks.** Turn the current manual comparison
   against `mpl/tests/examples` and `mpl/tests/errors` into a reproducible test
   harness with explicit expected divergences. Keep `mplc` a development oracle,
   not a runtime dependency.
2. **Complete grammar and HIR coverage.** Audit every documented and compiler-
   accepted query form, operator, recovery boundary, and ambiguity. Give `join`,
   `replace`, `ifdef`, `sample`, and parameter declarations dedicated semantic
   models instead of syntax-only handling.
3. **Richer semantic validation.** Add function arity and type checks, pipeline
   compatibility, alias/reference resolution, parameter type/default checks,
   time-range validation, stable diagnostic codes, and targeted suggestions.
4. **Incremental analysis database.** Cache files, parses, HIR, diagnostics, and
   IDE queries so edits do not re-run the complete analysis pipeline.
5. **Production formatter.** Define complete CST-aware layout rules, including
   comments, malformed input, line breaking, and stable/idempotent formatting
   across the full language.
6. **Semantic completions and navigation.** Use HIR for fields, tags, aliases,
   user parameters, valid pipeline operations, snippets, ranking, definitions,
   references, and rename.
7. **Expand structured documentation.** Grow the licensed and attributed
   function/keyword catalog used by completion, hover, and signature help,
   including richer examples and accurate parameter metadata.
8. **Expand code actions.** Beyond the existing diagnostic quick fixes for
   deprecated `filter`, lowercase `duration`, and unnecessary backticks, add
   targeted edits such as inserting a missing `using`, correcting an unknown
   pipeline operation, and qualifying a function name.
9. **LSP production work.** Add document versions, cancellation, tracing,
   workspace file handling, configuration, richer diagnostics, range formatting,
   and editor smoke tests.
