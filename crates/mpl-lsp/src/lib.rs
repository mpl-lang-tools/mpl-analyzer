//! JSON-RPC/LSP adapter for MPL analysis.
//!
//! This module owns transport, document storage, LSP capability negotiation,
//! UTF-16 position conversion, and mapping between `mpl-ide` results and
//! `lsp-types`. Parser, HIR, and editor feature logic stay in lower crates.

use std::{collections::HashMap, error::Error, fmt::Debug};

use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::{
    CompletionOptions, CompletionResponse, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, Documentation, HoverContents, HoverProviderCapability,
    InitializeParams, MarkupContent, MarkupKind, OneOf, ParameterInformation, ParameterLabel,
    Position, PublishDiagnosticsParams, Range, ServerCapabilities, SignatureHelpOptions,
    SignatureInformation, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification,
        PublishDiagnostics,
    },
    request::{Completion, Formatting, HoverRequest, Request as LspRequest, SignatureHelpRequest},
};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub fn run() -> Result<()> {
    let (connection, io_threads) = Connection::stdio();
    let init_value = serde_json::to_value(server_capabilities())?;

    let init_params = connection.initialize(init_value)?;
    main_loop(connection, init_params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: Connection, params: serde_json::Value) -> Result<()> {
    let _params: InitializeParams = serde_json::from_value(params)?;
    let mut documents = HashMap::new();

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                handle_request(&connection, request, &documents)?;
            }
            Message::Notification(notification) => {
                handle_notification(&connection, notification, &mut documents)?;
            }
            Message::Response(_) => {}
        }
    }

    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions::default()),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: Some(vec![",".to_string()]),
            ..Default::default()
        }),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

fn handle_notification(
    connection: &Connection,
    notification: lsp_server::Notification,
    documents: &mut HashMap<Uri, String>,
) -> Result<()> {
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;
            let uri = params.text_document.uri;
            let text = params.text_document.text;
            documents.insert(uri.clone(), text);
            publish_diagnostics(
                connection,
                &uri,
                documents.get(&uri).expect("inserted document"),
            )?;
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;
            let uri = params.text_document.uri;
            if let Some(change) = params.content_changes.into_iter().last() {
                documents.insert(uri.clone(), change.text);
                publish_diagnostics(
                    connection,
                    &uri,
                    documents.get(&uri).expect("changed document"),
                )?;
            }
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params)?;
            let uri = params.text_document.uri;
            documents.remove(&uri);
            publish_lsp_diagnostics(connection, uri, Vec::new())?;
        }
        _ => {}
    }

    Ok(())
}

fn handle_request(
    connection: &Connection,
    request: Request,
    documents: &HashMap<Uri, String>,
) -> Result<()> {
    match request.method.as_str() {
        Completion::METHOD => {
            let params: lsp_types::CompletionParams = serde_json::from_value(request.params)?;
            let uri = params.text_document_position.text_document.uri;
            let position = params.text_document_position.position;
            let items = documents
                .get(&uri)
                .map(|text| {
                    let offset = position_to_offset(text, position);
                    mpl_ide::completions(text, offset)
                        .into_iter()
                        .map(|item| completion_item_to_lsp(text, item))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            send_ok(
                connection,
                request.id,
                serde_json::to_value(CompletionResponse::Array(items))?,
            )?;
        }
        HoverRequest::METHOD => {
            let params: lsp_types::HoverParams = serde_json::from_value(request.params)?;
            let uri = params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;
            let hover = documents.get(&uri).and_then(|text| {
                let offset = position_to_offset(text, position);
                mpl_ide::hover(text, offset).map(|hover| hover_to_lsp(text, hover))
            });
            send_ok(connection, request.id, serde_json::to_value(hover)?)?;
        }
        SignatureHelpRequest::METHOD => {
            let params: lsp_types::SignatureHelpParams = serde_json::from_value(request.params)?;
            let uri = params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;
            let signature_help = documents.get(&uri).and_then(|text| {
                let offset = position_to_offset(text, position);
                mpl_ide::signature_help(text, offset).map(signature_help_to_lsp)
            });
            send_ok(
                connection,
                request.id,
                serde_json::to_value(signature_help)?,
            )?;
        }
        Formatting::METHOD => {
            let params: DocumentFormattingParams = serde_json::from_value(request.params)?;
            let uri = params.text_document.uri;
            let edits = documents
                .get(&uri)
                .map(|text| {
                    let formatted = mpl_ide::format(text);
                    if formatted == *text {
                        Vec::new()
                    } else {
                        vec![TextEdit {
                            range: full_document_range(text),
                            new_text: formatted,
                        }]
                    }
                })
                .unwrap_or_default();
            send_ok(connection, request.id, serde_json::to_value(edits)?)?;
        }
        _ => send_error(
            connection,
            request.id,
            lsp_server::ErrorCode::MethodNotFound,
            "method not supported",
        )?,
    }

    Ok(())
}

fn publish_diagnostics(connection: &Connection, uri: &Uri, text: &str) -> Result<()> {
    let diagnostics = mpl_ide::diagnostics(text)
        .into_iter()
        .map(|diagnostic| Diagnostic {
            range: text_range_to_lsp(text, diagnostic.range.start, diagnostic.range.end),
            severity: Some(diagnostic_severity(&diagnostic.severity)),
            code: None,
            code_description: None,
            source: Some("mpl-analyzer".to_string()),
            message: diagnostic.message,
            related_information: None,
            tags: None,
            data: None,
        })
        .collect();
    publish_lsp_diagnostics(connection, uri.clone(), diagnostics)
}

fn diagnostic_severity(severity: &impl Debug) -> DiagnosticSeverity {
    match format!("{severity:?}").as_str() {
        "Error" => DiagnosticSeverity::ERROR,
        "Warning" => DiagnosticSeverity::WARNING,
        "Hint" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::INFORMATION,
    }
}

fn publish_lsp_diagnostics(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<Diagnostic>,
) -> Result<()> {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    connection
        .sender
        .send(Message::Notification(lsp_server::Notification::new(
            PublishDiagnostics::METHOD.to_string(),
            params,
        )))?;
    Ok(())
}

fn send_ok(connection: &Connection, id: RequestId, result: serde_json::Value) -> Result<()> {
    connection.sender.send(Message::Response(Response {
        id,
        result: Some(result),
        error: None,
    }))?;
    Ok(())
}

fn send_error(
    connection: &Connection,
    id: RequestId,
    code: lsp_server::ErrorCode,
    message: &str,
) -> Result<()> {
    connection.sender.send(Message::Response(Response {
        id,
        result: None,
        error: Some(lsp_server::ResponseError {
            code: code as i32,
            message: message.to_string(),
            data: None,
        }),
    }))?;
    Ok(())
}

fn text_range_to_lsp(text: &str, start: usize, end: usize) -> Range {
    Range::new(
        offset_to_position(text, start),
        offset_to_position(text, end),
    )
}

fn completion_item_to_lsp(text: &str, item: mpl_ide::CompletionItem) -> lsp_types::CompletionItem {
    let edit = TextEdit {
        range: text_range_to_lsp(
            text,
            item.replacement_range.start,
            item.replacement_range.end,
        ),
        new_text: item.label.clone(),
    };
    lsp_types::CompletionItem {
        label: item.label,
        detail: item.detail,
        text_edit: Some(edit.into()),
        ..Default::default()
    }
}

fn hover_to_lsp(text: &str, hover: mpl_ide::Hover) -> lsp_types::Hover {
    lsp_types::Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents,
        }),
        range: Some(text_range_to_lsp(text, hover.range.start, hover.range.end)),
    }
}

fn signature_help_to_lsp(help: mpl_ide::SignatureHelp) -> lsp_types::SignatureHelp {
    let parameters = help
        .parameters
        .into_iter()
        .map(|parameter| ParameterInformation {
            label: ParameterLabel::LabelOffsets([
                signature_offset_to_utf16(&help.signature, parameter.label.start),
                signature_offset_to_utf16(&help.signature, parameter.label.end),
            ]),
            documentation: parameter.documentation.map(|value| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value,
                })
            }),
        })
        .collect::<Vec<_>>();
    let active_parameter = help.active_parameter.map(|index| index as u32);
    lsp_types::SignatureHelp {
        signatures: vec![SignatureInformation {
            label: help.signature,
            documentation: help.documentation.map(|value| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value,
                })
            }),
            parameters: (!parameters.is_empty()).then_some(parameters),
            active_parameter,
        }],
        active_signature: Some(0),
        active_parameter,
    }
}

fn signature_offset_to_utf16(signature: &str, byte_offset: usize) -> u32 {
    signature[..byte_offset].encode_utf16().count() as u32
}

fn full_document_range(text: &str) -> Range {
    Range::new(Position::new(0, 0), offset_to_position(text, text.len()))
}

fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }

    let mut line = 0;
    let mut character = 0;
    for ch in text[..offset].chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }

    Position::new(line, character)
}

fn position_to_offset(text: &str, position: Position) -> usize {
    let mut line = 0;
    let mut line_start = 0;

    for (idx, ch) in text.char_indices() {
        if line == position.line {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }

    if line < position.line {
        return text.len();
    }

    let mut utf16_character = 0;
    for (relative_idx, ch) in text[line_start..].char_indices() {
        if ch == '\n' || utf16_character >= position.character {
            return line_start + relative_idx;
        }

        let next_character = utf16_character + ch.len_utf16() as u32;
        if next_character > position.character {
            return line_start + relative_idx;
        }
        utf16_character = next_character;
    }

    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_use_full_sync_and_requested_features() {
        let capabilities = server_capabilities();

        assert!(matches!(
            capabilities.text_document_sync,
            Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
        ));
        assert!(capabilities.completion_provider.is_some());
        assert!(matches!(
            capabilities.hover_provider,
            Some(HoverProviderCapability::Simple(true))
        ));
        assert!(capabilities.signature_help_provider.is_some());
        assert!(matches!(
            capabilities.document_formatting_provider,
            Some(OneOf::Left(true))
        ));
    }

    #[test]
    fn initialize_value_is_server_capabilities_not_initialize_result() {
        let value = serde_json::to_value(server_capabilities()).unwrap();
        assert!(value.get("completionProvider").is_some());
        assert!(value.get("capabilities").is_none());
    }

    #[test]
    fn maps_ide_diagnostic_severities_to_lsp() {
        #[derive(Debug)]
        enum IdeSeverity {
            Error,
            Warning,
            Hint,
            Unknown,
        }

        assert_eq!(
            diagnostic_severity(&IdeSeverity::Error),
            DiagnosticSeverity::ERROR
        );
        assert_eq!(
            diagnostic_severity(&IdeSeverity::Warning),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            diagnostic_severity(&IdeSeverity::Hint),
            DiagnosticSeverity::HINT
        );
        assert_eq!(
            diagnostic_severity(&IdeSeverity::Unknown),
            DiagnosticSeverity::INFORMATION
        );
    }

    #[test]
    fn converts_offsets_to_utf16_positions() {
        let text = "a\n😀b";

        assert_eq!(offset_to_position(text, 0), Position::new(0, 0));
        assert_eq!(offset_to_position(text, 2), Position::new(1, 0));
        assert_eq!(offset_to_position(text, 6), Position::new(1, 2));
        assert_eq!(offset_to_position(text, text.len()), Position::new(1, 3));
    }

    #[test]
    fn hover_uses_markdown_markup_content_and_source_range() {
        let text = "from prod:requests | map fill::const(0)";
        let hover = mpl_ide::hover(text, 31).unwrap();
        let hover = hover_to_lsp(text, hover);

        assert_eq!(
            hover.contents,
            HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "`fill::const(value)`\n\nFill missing values with a constant.".into(),
            })
        );
        assert_eq!(
            hover.range,
            Some(Range::new(Position::new(0, 25), Position::new(0, 36),))
        );
    }

    #[test]
    fn signature_help_uses_markdown_documentation() {
        let text = "from prod:requests | map fill::const()";
        let help = mpl_ide::signature_help(text, 37).unwrap();
        let help = signature_help_to_lsp(help);
        let signature = &help.signatures[0];

        assert_eq!(signature.label, "fill::const(value)");
        assert_eq!(
            signature.documentation,
            Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Fill missing values with a constant.".into(),
            }))
        );
        assert_eq!(signature.active_parameter, Some(0));
        assert_eq!(help.active_parameter, Some(0));
        assert_eq!(
            signature.parameters,
            Some(vec![ParameterInformation {
                label: ParameterLabel::LabelOffsets([12, 17]),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "The constant used to replace missing values.".into(),
                })),
            }])
        );
    }

    #[test]
    fn signature_help_maps_later_arguments_to_variadic_parameter() {
        let text = "from prod:requests | bucket by le using interpolate_cumulative_histogram(linear, 0.5, 0.9)";
        let help = mpl_ide::signature_help(text, 87).unwrap();
        let help = signature_help_to_lsp(help);
        let signature = &help.signatures[0];

        assert_eq!(signature.active_parameter, Some(1));
        assert_eq!(help.active_parameter, Some(1));
        assert_eq!(
            signature
                .parameters
                .as_ref()
                .unwrap()
                .iter()
                .map(|parameter| parameter.label.clone())
                .collect::<Vec<_>>(),
            vec![
                ParameterLabel::LabelOffsets([33, 37]),
                ParameterLabel::LabelOffsets([39, 47]),
            ]
        );
    }

    #[test]
    fn completion_edit_replaces_partial_prefix_with_full_label() {
        let text = "from prod:requests | m    rate";
        let item = mpl_ide::completions(text, 22).into_iter().next().unwrap();
        let item = completion_item_to_lsp(text, item);

        assert_eq!(item.label, "map");
        assert_eq!(
            item.text_edit,
            Some(
                TextEdit {
                    range: Range::new(Position::new(0, 21), Position::new(0, 22)),
                    new_text: "map".into(),
                }
                .into()
            )
        );
    }

    #[test]
    fn completion_edit_inserts_at_cursor_without_a_prefix() {
        let text = "from prod:requests | ";
        let item = mpl_ide::completions(text, text.len())
            .into_iter()
            .next()
            .unwrap();
        let item = completion_item_to_lsp(text, item);
        let cursor = text.len() as u32;

        assert_eq!(
            item.text_edit,
            Some(
                TextEdit {
                    range: Range::new(Position::new(0, cursor), Position::new(0, cursor)),
                    new_text: item.label,
                }
                .into()
            )
        );
    }

    #[test]
    fn converts_utf16_positions_to_offsets() {
        let text = "a\n😀b";

        assert_eq!(position_to_offset(text, Position::new(0, 0)), 0);
        assert_eq!(position_to_offset(text, Position::new(1, 0)), 2);
        assert_eq!(position_to_offset(text, Position::new(1, 2)), 6);
        assert_eq!(position_to_offset(text, Position::new(1, 3)), text.len());
        assert_eq!(position_to_offset(text, Position::new(3, 0)), text.len());
    }
}
