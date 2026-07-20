# Provenance

This project is an MPL analyzer implementation. It is not developed under a
clean-room restriction: appropriately licensed MPL code and other material may
be reused when that is the clearest implementation path.

Public sources consulted for behavioral compatibility:

- [Axiom MPL introduction](https://axiom.co/docs/mpl/introduction)
- [Axiom sample MPL queries](https://axiom.co/docs/mpl/sample-queries)
- [Axiom PromQL migration notes](https://axiom.co/docs/mpl/migrate-metrics)
- [Axiom MPL API endpoint](https://axiom.co/docs/restapi/endpoints/queryMetrics)

The canonical implementation reference is
[`axiomhq/mpl`](https://github.com/axiomhq/mpl). A local checkout or a fork that
tracks that repository may be used when available; tooling and contributor
instructions should discover and verify local checkouts rather than assume a
fixed filesystem path.

Reference material consulted from that repository includes:

- `spec.md`
- `src/mpl.pest`
- `src/parser.rs` and `src/query.rs` warning behavior
- `tests/examples`
- `tests/errors`
- `extra/mpl-language-server/src/diagnostics.rs` and `src/lints.rs`
- `packages/mpl-codemirror/src/diagnostics.ts`

The example and error corpora are used as behavioral parity inputs: valid
fixtures should remain free of error-level diagnostics and invalid fixtures
should produce an analyzer error. Valid deprecated or non-canonical input may
produce warnings or hints. Compatibility is evaluated from observable outcomes
rather than by importing the compiler or its grammar into the analyzer.

Architectural influence:

- rust-analyzer's public architecture pattern: lexer, lossless syntax tree,
  typed AST wrappers, HIR, IDE APIs, and LSP adapter.

## MPL licensing

The `axiomhq/mpl` repository declares `MIT OR Apache-2.0`. Contributors may
copy, modify, and integrate MPL material under either license, subject to that
license's conditions.

This repository is distributed under MIT and, by default, uses incorporated MPL
material under MPL's MIT option. The upstream notice is reproduced in
`THIRD_PARTY_NOTICES.md`. Any MPL material used under Apache-2.0 instead must be
identified explicitly.

The repository's own MIT notice names the MPL contributors as its copyright
holder. That project notice is separate from, and does not replace, Axiom's
retained notice for MPL material incorporated from `axiomhq/mpl`.

For copied or substantially derived material:

- record the upstream repository, file, and revision;
- identify the selected license;
- retain applicable copyright, license, patent, trademark, and attribution
  notices; and
- mark modifications when required by the selected license.

Under the MIT option, distributions containing copies or substantial portions
must include the upstream MIT copyright and permission notice. Under the
Apache-2.0 option, distributions must satisfy its redistribution conditions,
including providing the license, retaining applicable notices, marking modified
files, and carrying forward any applicable NOTICE content.

License review is source-specific. The MPL license does not grant permission to
copy material from rust-analyzer or any other project under different terms;
their licenses must be checked and followed separately.
