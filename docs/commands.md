# CLI Commands

`mpl-analyzer` reads MPL source from a file and writes results to stdout.

## Syntax

```text
mpl-analyzer <parse|check|format> <file>
mpl-analyzer <complete|hover|signature> <file> <offset>
```

`offset` is a zero-based byte offset into the file. Offset-based commands call
the same IDE APIs used by the language server.

## Commands

- `parse <file>`: prints the lossless CST debug tree followed by syntax
  diagnostics.
- `check <file>`: prints diagnostics as pretty JSON.
- `format <file>`: prints formatted MPL source.
- `complete <file> <offset>`: prints completion items as pretty JSON.
- `hover <file> <offset>`: prints hover information as pretty JSON, or `null`.
- `signature <file> <offset>`: prints signature help as pretty JSON, or `null`.
