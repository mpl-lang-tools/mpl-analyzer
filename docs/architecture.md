# Architecture

`mpl-analyzer` is an IDE-first implementation for MPL. The compiler or query
engine is not the source of editor truth; the analyzer owns partial syntax,
recovery, source ranges, formatting, completions, and code actions.

The final intended shape follows the same broad architecture style as
rust-analyzer:

```text
source text
  -> lexer
  -> lossless rowan CST
  -> typed AST wrappers
  -> HIR
  -> IDE APIs
  -> LSP adapter
```

This is an architectural influence, not a license to copy implementation.
`mpl-analyzer` must not copy rust-analyzer source code, generated code,
grammar definitions, tests, or prose verbatim. Use public concepts and design
patterns only, then implement MPL-specific code in this repository.

## Layers

### Lexer

The lexer turns source text into tokens while preserving exact text spans. It
should be trivia-aware: whitespace, comments, and unknown tokens still matter
because later layers need stable ranges, recovery, and formatting input.

### Lossless CST

Parsing produces a concrete syntax tree backed by `rowan`. The CST is lossless:
all tokens and trivia from the original file remain represented, including
comments, whitespace, incomplete constructs, and syntax errors.

The CST matters because IDE features operate on live, partially typed files.
Users expect diagnostics, completions, hover, formatting, and code actions to
work before the file is semantically valid. A lossy parser would discard the
exact source structure needed for source ranges, incremental reparsing,
round-trip formatting, and precise edits.

### Typed AST wrappers

Typed AST wrappers provide a convenient, checked view over CST nodes and tokens.
They should be thin wrappers: no semantic meaning, name resolution, type
checking, or validation belongs here. Their job is to make syntax traversal
ergonomic while keeping the underlying tree lossless.

### HIR

HIR lowers syntax into semantic MPL structures. This layer owns name-like
resolution, function and transformation validation, semantic diagnostics, and
normal forms that IDE features can query without depending on concrete syntax
details.

HIR should keep source mappings back to CST/AST ranges so semantic diagnostics
and assists can point at the right text.

### IDE

The IDE layer is the user-facing analysis API. It combines syntax and HIR to
provide diagnostics, lints, completions, hover, signature help, formatting, and
future code actions. It should expose plain Rust APIs that are independent of
JSON-RPC and LSP types.

### LSP

The LSP layer is only a protocol adapter. It owns document synchronization,
position encoding conversion, request/notification routing, and conversion
between IDE results and `lsp-types`. It should not reimplement parser,
semantic, or completion logic.

## Crate Responsibilities

- `mpl-syntax`: owns lexical tokens, rowan language integration, lossless CST
  construction, typed AST wrappers, syntax diagnostics, and syntax-preserving
  formatting helpers.
- `mpl-hir`: owns lowering from typed CST/AST wrappers into semantic MPL
  structures, semantic validation, semantic diagnostics, and source mappings.
- `mpl-ide`: owns editor features such as merged diagnostics, lints,
  completions, hover, signature help, formatting entry points, and future code
  actions.
- `mpl-lsp`: owns JSON-RPC/LSP transport, document storage, UTF-16/byte range
  conversion, capability advertisement, and request adaptation to `mpl-ide`.
- `mpl-cli`: owns command-line debug and smoke-test entry points for parse,
  check, format, complete, hover, and signature behavior.

Dependencies should flow in one direction:

```text
mpl-cli -> mpl-ide -> mpl-hir -> mpl-syntax
mpl-lsp -> mpl-ide -> mpl-hir -> mpl-syntax
```

No lower layer should depend on an upper layer. In particular, syntax must not
know about HIR, IDE, CLI, or LSP, and IDE behavior should be testable without
starting the language server.

## Test Strategy

Use `insta` snapshots for stable, reviewable outputs at each layer.

- `mpl-syntax` snapshots should cover tokenization-sensitive parse trees,
  syntax diagnostics, recovery cases, and formatter output.
- `mpl-hir` snapshots should cover lowering results, semantic diagnostics,
  syntax diagnostic pass-through, and source mapping behavior.
- `mpl-ide` snapshots should cover diagnostics, completions, hover, signature
  help, formatting edits, and future assists from representative cursor
  positions.
- `mpl-lsp` tests should focus on protocol adaptation: position conversions,
  document synchronization, capability shape, and conversion of IDE results into
  LSP responses.
- `mpl-cli` tests should stay thin and verify command wiring, argument handling,
  and output stability.

Snapshot fixtures should include valid public MPL examples, incomplete files,
malformed constructs, comments and whitespace-heavy inputs, and cursor-position
cases. When behavior changes intentionally, update snapshots in the same change
and make the semantic reason clear in the review.

Command details are documented in `docs/commands.md`. LSP transport behavior
and advertised capabilities are documented in `docs/lsp.md`.
