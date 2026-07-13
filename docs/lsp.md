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
ranges and severities before publication.

## Requests

- `textDocument/completion`: returns IDE completion items for the current
  document and position.
- `textDocument/hover`: returns IDE hover contents and range when available.
- `textDocument/signatureHelp`: returns IDE signature help when available.
- `textDocument/formatting`: returns a single full-document text edit when
  formatting changes the document, or an empty edit list when it is already
  formatted.

Positions from the client are interpreted as LSP UTF-16 positions and converted
to the byte offsets used by the IDE layer. Internal byte ranges are converted
back to UTF-16 LSP ranges before being sent to clients.
