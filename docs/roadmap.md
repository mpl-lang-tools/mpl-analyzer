# Roadmap

Major Work Left

  1. Complete MPL grammar coverage
     Current parser covers representative/public examples, but we need a full grammar pass against MPL docs: every query form, operator, function syntax, edge
     case, recovery case, and ambiguous construct.
  2. Incremental database layer
     rust-analyzer’s real power comes from salsa-style incremental queries. We do not yet have a database for files, parse results, HIR, diagnostics, or IDE
     queries.
  3. Richer semantic validation
     Need type-ish checking, function arity/signature validation, pipe compatibility, alias/reference resolution, parameter validation, and better diagnostic
     codes.
  4. Production formatter
     Current formatting is basic. It needs a proper CST-aware formatter with stable layout rules, comments/trivia handling, idempotency tests, and malformed-code
     behavior.
  5. Completions need semantic awareness
     Current completions are context-based. They should use HIR for functions, fields/tags, aliases, params, pipe-specific valid items, snippets, and ranking.
  6. Hover/signature need real docs
     Need a structured stdlib/docs model sourced from clean-room MPL documentation notes, not copied prose, so hover and signatures are useful.
  7. Code actions
     Not implemented meaningfully yet. Needed examples: replace filter with where, insert missing using, fix unknown pipe typo, qualify function names, format
     selection, organize/rewrite constructs.
  8. LSP polish
     Need document-change robustness, cancellation, better logging, trace support, workspace file handling, config, diagnostic versioning, and Neovim/VSCodium
     smoke tests.
