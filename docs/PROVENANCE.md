# Provenance

This project is an independent MPL analyzer implementation.

Sources consulted for behavioral compatibility:

- Axiom MPL introduction: https://axiom.co/docs/mpl/introduction
- Axiom sample MPL queries: https://axiom.co/docs/mpl/sample-queries
- Axiom PromQL migration notes: https://axiom.co/docs/mpl/migrate-metrics
- Axiom MPL API endpoint: https://axiom.co/docs/restapi/endpoints/queryMetrics

Local reference material consulted, but not copied:

- `/home/samu/repos/samu/mpl/spec.md`
- `/home/samu/repos/samu/mpl/src/mpl.pest`
- `/home/samu/repos/samu/mpl/tests/examples`
- `/home/samu/repos/samu/mpl/extra/mpl-language-server`

Architectural influence:

- rust-analyzer's public architecture pattern: lexer, lossless syntax tree,
  typed AST wrappers, HIR, IDE APIs, and LSP adapter.

Implementation rule: source code, grammar, docs, and fixtures in this repository
must be original work. Public docs and local upstream files may be used to
extract language facts, but prose, grammar productions, code, and examples
should not be copied verbatim.

The same rule applies to rust-analyzer. Its public architecture may inform this
project's design, but rust-analyzer source code, generated code, tests, grammar
definitions, and prose must not be copied verbatim.
