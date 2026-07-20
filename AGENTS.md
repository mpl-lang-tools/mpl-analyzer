# Repository Instructions

## Architecture

- Keep dependencies flowing from CLI/LSP through IDE and HIR to syntax. Lower
  layers must not depend on upper layers.
- Keep JSON-RPC and LSP types in `mpl-lsp`; `mpl-ide` exposes protocol-neutral
  Rust APIs.
- Keep `mpl-analyzer` standalone. Do not add an MPL compiler runtime dependency,
  vendored compiler, or compiler submodule unless the user explicitly requests
  that architecture.

## MPL compatibility

- The canonical behavioral reference is
  [`axiomhq/mpl`](https://github.com/axiomhq/mpl). Treat `mplc` as a development
  oracle, not as part of the analyzer runtime.
- Before fetching or cloning the reference repository, look for an existing
  local checkout or worktree in nearby workspace/repository directories. Verify
  candidates with `git remote -v`; a fork that tracks `axiomhq/mpl` is suitable.
  Do not assume a fixed relative path.
- MPL is dual-licensed under MIT or Apache-2.0. Code, grammar, tests, and other
  material may be reused or adapted when doing so is useful, provided the
  selected license and its attribution and redistribution requirements are
  followed.
- This repository is MIT-licensed and normally uses MPL material under MPL's
  MIT option. Preserve Axiom's notice in `THIRD_PARTY_NOTICES.md`. If material
  is instead used under Apache-2.0, document that choice and satisfy the Apache
  redistribution requirements.
- For copied or substantially derived material, record the upstream file and
  revision and preserve applicable copyright, license, and attribution notices.
  Do not describe reused material as independently implemented.
- When comparing behavior, check both accepted examples and rejected/error
  fixtures. Document deliberate divergences.

## Tests and snapshots

- Keep one behavior case per snapshot. Do not combine unrelated cases into a
  single snapshot.
- Include incomplete input and code before and after cursor positions for IDE
  features where relevant.
- Update snapshots only for intentional behavior changes and inspect the
  rendered result before accepting it.
- Use `mpl-code-render` for human-readable source annotations in IDE snapshots.

## Verification

- Run `cargo fmt --all -- --check`, `cargo test --workspace`, and
  `git diff --check` for repository changes.
- Run strict Clippy on the crates changed by the task. Do not expand the task to
  unrelated existing workspace warnings without noting them.
- Preserve unrelated working-tree and staged changes.
