# LSP Capabilities

`mpl-lsp` is the JSON-RPC language server for MPL. It keeps an in-memory copy
of open documents, uses full document synchronization, and delegates language
analysis to `mpl-ide`.

## Synchronization

- `textDocument/didOpen`: stores the full document and publishes diagnostics.
- `textDocument/didChange`: expects full document changes, replaces the stored
  document text, and publishes diagnostics.
- `textDocument/didClose`: removes the stored document and clears diagnostics.

Diagnostics are requested from `mpl-ide` and translated to LSP diagnostic
ranges and severities before publication. Diagnostic help is appended after the
primary message, matching the CodeMirror presentation. Published diagnostics
currently omit document versions, diagnostic codes, related information, tags,
and data.

## Requests

- `textDocument/completion`: returns IDE completion items for the current
  document and position. Every item carries a `textEdit` replacement range, so
  partially typed prefixes are replaced by the full completion label.
- `textDocument/hover`: returns Markdown contents and the annotated source range
  when available.
- `textDocument/signatureHelp`: returns one signature with Markdown
  documentation, parameter label offsets, and the active parameter when
  available. `(` and `,` are advertised as trigger characters, and `,` as a
  retrigger character.
- `textDocument/formatting`: returns a single full-document text edit when
  formatting changes the document, or an empty edit list when it is already
  formatted.
- `textDocument/codeAction`: returns quick-fix replacement edits attached to
  diagnostics, currently replacing deprecated `filter`, lowercase `duration`
  parameter types, and unnecessary identifier backticks. Only actions whose
  edit range intersects the requested range are returned.

Positions from the client are interpreted as LSP UTF-16 positions and converted
to the byte offsets used by the IDE layer. Internal byte ranges are converted
back to UTF-16 LSP ranges before being sent to clients.

Requests for documents that are not open return an empty completion,
formatting, or code-action result, or `null` hover/signature help. The server
currently has no workspace index and does not read unopened files on demand.

## Not advertised

The server does not currently advertise incremental synchronization, rename,
definitions/references, symbols, semantic tokens, inlay hints, workspace
configuration, or range formatting.
