# Architecture

`mpl-analyzer` is a standalone, IDE-first implementation of MPL. It owns
partial syntax, recovery, source ranges, diagnostics, formatting, completions,
hover, and signature help. It does not invoke or embed `mplc` at runtime.

The MPL implementation and its public examples remain compatibility references.
The analyzer should agree with `mplc` where practical while retaining the
error-tolerant syntax model needed by editors.

The implementation follows the same broad architecture style as
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

Architectural influence and code reuse are separate questions. Before reusing
rust-analyzer or any other project's implementation, generated code, tests,
grammar, or prose, verify its license and preserve all required notices and
attribution. Record substantially copied or derived material in the provenance
documentation.

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
exact source structure needed for source ranges, future incremental reparsing,
round-trip formatting, and precise edits.

### Typed AST wrappers

Typed AST wrappers provide a convenient, checked view over CST nodes and tokens.
They should be thin wrappers: no semantic meaning, name resolution, type
checking, or validation belongs here. Their job is to make syntax traversal
ergonomic while keeping the underlying tree lossless.

### HIR

HIR lowers supported syntax into semantic MPL structures. This layer owns
parameter checks, function and transformation validation, semantic diagnostics,
and normal forms that IDE features can query without depending on concrete
syntax details. Syntax support may precede HIR support; constructs that are only
recognized for recovery remain explicit unknown pipes after lowering.

HIR should keep source mappings back to CST/AST ranges so semantic diagnostics
and assists can point at the right text.

### IDE

The IDE layer is the user-facing analysis API. It combines syntax and HIR to
provide diagnostics, lints, completions with replacement ranges, hover,
signature help, formatting, and future code actions. It exposes plain Rust APIs
that are independent of JSON-RPC and LSP types.

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
- `mpl-code-render`: owns source excerpts and point/span annotations used by
  human-readable IDE snapshots. It is a presentation helper, not an analysis
  layer.
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
cases. Keep one behavior case per snapshot so failures identify a single
contract. When behavior changes intentionally, update snapshots in the same
change and make the semantic reason clear in the review.

Command details are documented in `docs/commands.md`. LSP transport behavior
and advertised capabilities are documented in `docs/lsp.md`.
