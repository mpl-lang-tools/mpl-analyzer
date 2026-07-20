# CLI Commands

`mpl-analyzer` reads MPL source from a file and writes results to stdout.

## Syntax

```text
mpl-analyzer <parse|check|format> <file>
mpl-analyzer <complete|hover|signature> <file> <offset>
```

`offset` is a zero-based byte offset into the file. Offset-based commands call
the same IDE APIs used by the language server. Pass an offset at a UTF-8
character boundary. LSP clients do not use byte offsets directly; the server
converts their UTF-16 positions.

## Commands

- `parse <file>`: prints the lossless CST debug tree followed by the parser's
  detailed syntax diagnostics.
- `check <file>`: prints analyzer diagnostics as pretty JSON. Each diagnostic
  contains `severity`, `message`, optional `help`, replacement `actions`, and a
  byte `range` with `start` and `end`.
- `format <file>`: prints formatted MPL source.
- `complete <file> <offset>`: prints completion items as pretty JSON. Items
  contain their full label, detail, and the byte range the label replaces.
- `hover <file> <offset>`: prints a byte range and Markdown contents as pretty
  JSON, or `null`.
- `signature <file> <offset>`: prints one signature, its documentation,
  parameter label ranges, variadic flags, and the active parameter as pretty
  JSON, or `null`.

Argument errors exit with status 2. File and serialization errors exit with
status 1. Analyzer diagnostics are data returned by `check`; their presence
does not currently change the process exit status.
