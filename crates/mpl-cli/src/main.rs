//! Command-line entry point for inspecting analyzer behavior without an editor.
//!
//! This binary keeps CLI concerns separate from the IDE and LSP layers. It is
//! intentionally thin: commands parse files, call the same public analysis APIs
//! used by the language server, and print stable debug output for smoke tests.

use std::{env, fs, process};

const USAGE: &str = "\
usage:
  mpl-analyzer <parse|check|format> <file>
  mpl-analyzer <complete|hover|signature> <file> <offset>

offset is a zero-based byte offset";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandKind {
    Parse,
    Check,
    Format,
    Complete,
    Hover,
    Signature,
}

#[derive(Debug, PartialEq, Eq)]
struct Command {
    kind: CommandKind,
    path: String,
    offset: Option<usize>,
}

fn main() {
    let command = parse_args(env::args().skip(1)).unwrap_or_else(|err| {
        eprintln!("{err}");
        eprintln!("{USAGE}");
        process::exit(2);
    });

    let input = fs::read_to_string(&command.path).unwrap_or_else(|err| {
        eprintln!("{}: {err}", command.path);
        process::exit(1);
    });

    if let Err(err) = run_command(command, &input) {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let command = args.next().ok_or_else(|| "missing command".to_string())?;
    let kind = match command.as_str() {
        "parse" => CommandKind::Parse,
        "check" => CommandKind::Check,
        "format" => CommandKind::Format,
        "complete" => CommandKind::Complete,
        "hover" => CommandKind::Hover,
        "signature" => CommandKind::Signature,
        _ => return Err(format!("unknown command: {command}")),
    };

    let path = args.next().ok_or_else(|| "missing file".to_string())?;
    let needs_offset = matches!(
        kind,
        CommandKind::Complete | CommandKind::Hover | CommandKind::Signature
    );
    let offset = if needs_offset {
        let raw_offset = args.next().ok_or_else(|| "missing offset".to_string())?;
        Some(
            raw_offset
                .parse()
                .map_err(|_| format!("invalid offset: {raw_offset}"))?,
        )
    } else {
        None
    };

    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument: {extra}"));
    }

    Ok(Command { kind, path, offset })
}

fn run_command(command: Command, input: &str) -> Result<(), serde_json::Error> {
    match command.kind {
        CommandKind::Parse => {
            print!("{}", render_parse_output(input));
        }
        CommandKind::Check => {
            println!(
                "{}",
                serde_json::to_string_pretty(&mpl_ide::diagnostics(input))?
            );
        }
        CommandKind::Format => {
            print!("{}", mpl_ide::format(input));
        }
        CommandKind::Complete => {
            println!(
                "{}",
                serde_json::to_string_pretty(&mpl_ide::completions(
                    input,
                    command.offset.expect("validated offset")
                ))?
            );
        }
        CommandKind::Hover => {
            println!(
                "{}",
                serde_json::to_string_pretty(&mpl_ide::hover(
                    input,
                    command.offset.expect("validated offset")
                ))?
            );
        }
        CommandKind::Signature => {
            println!(
                "{}",
                serde_json::to_string_pretty(&mpl_ide::signature_help(
                    input,
                    command.offset.expect("validated offset")
                ))?
            );
        }
    }

    Ok(())
}

fn render_parse_output(input: &str) -> String {
    let parse = mpl_syntax::parse_syntax(input);
    format!(
        "{}Syntax diagnostics:\n{:#?}\n",
        mpl_syntax::debug_tree(parse.syntax()),
        parse.diagnostics()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn parses_file_command_without_offset() {
        assert_eq!(
            parse(&["check", "query.mpl"]),
            Ok(Command {
                kind: CommandKind::Check,
                path: "query.mpl".to_string(),
                offset: None,
            })
        );
    }

    #[test]
    fn parses_offset_command() {
        assert_eq!(
            parse(&["hover", "query.mpl", "12"]),
            Ok(Command {
                kind: CommandKind::Hover,
                path: "query.mpl".to_string(),
                offset: Some(12),
            })
        );
    }

    #[test]
    fn rejects_missing_offset() {
        assert_eq!(
            parse(&["complete", "query.mpl"]),
            Err("missing offset".to_string())
        );
    }

    #[test]
    fn rejects_extra_argument() {
        assert_eq!(
            parse(&["format", "query.mpl", "0"]),
            Err("unexpected argument: 0".to_string())
        );
    }

    #[test]
    fn renders_lossless_cst_and_syntax_diagnostics_for_parse() {
        let output = render_parse_output("http:requests[5m]\n| where service \"api\"\n");

        assert!(output.starts_with("Root\n"));
        assert!(output.contains("SimpleQuery\n"));
        assert!(output.contains("Ident \"http\"\n"));
        assert!(output.contains("Syntax diagnostics:\n"));
        assert!(output.contains("expected comparison operator"));
        assert!(!output.trim_start().starts_with('{'));
    }
}
