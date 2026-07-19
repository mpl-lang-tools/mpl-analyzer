//! Lightweight source-code rendering with point and span annotations.

use std::error::Error;
use std::fmt::{self, Write};
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

/// A zero-based source position.
///
/// `character` counts Unicode scalar values, not bytes, UTF-16 code units, or
/// terminal cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

impl Position {
    pub const fn new(line: usize, character: usize) -> Self {
        Self { line, character }
    }

    /// Converts this position to a UTF-8 byte offset in `source`.
    pub fn to_byte_offset(self, source: &str) -> Result<usize, RenderError> {
        let line = source
            .split('\n')
            .nth(self.line)
            .ok_or(RenderError::LineOutOfBounds { line: self.line })?;
        let line_start = source
            .split_inclusive('\n')
            .take(self.line)
            .map(str::len)
            .sum::<usize>();
        let line_offset = character_byte_offset(line, self)?;
        Ok(line_start + line_offset)
    }

    /// Converts a UTF-8 byte offset in `source` to a source position.
    pub fn from_byte_offset(source: &str, offset: usize) -> Result<Self, RenderError> {
        if offset > source.len() || !source.is_char_boundary(offset) {
            return Err(RenderError::InvalidByteOffset { offset });
        }

        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let character = source[line_start..offset].chars().count();
        Ok(Self::new(line, character))
    }
}

/// A source annotation rendered below its source line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotation<'a> {
    target: AnnotationTarget,
    label: &'a str,
    boxed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AnnotationTarget {
    Point(Position),
    Span(Range<Position>),
    AnchoredSpan {
        range: Range<Position>,
        anchor: Position,
    },
}

impl<'a> Annotation<'a> {
    /// Creates a zero-width annotation rendered with a box-drawing marker.
    pub const fn point(position: Position, label: &'a str) -> Self {
        Self {
            target: AnnotationTarget::Point(position),
            label,
            boxed: false,
        }
    }

    /// Creates a same-line annotation rendered with an underline.
    pub fn span(range: Range<Position>, label: &'a str) -> Self {
        Self {
            target: AnnotationTarget::Span(range),
            label,
            boxed: false,
        }
    }

    /// Creates a same-line underline whose label connector is placed at `anchor`.
    pub fn anchored_span(range: Range<Position>, anchor: Position, label: &'a str) -> Self {
        Self {
            target: AnnotationTarget::AnchoredSpan { range, anchor },
            label,
            boxed: false,
        }
    }

    /// Renders the annotation label inside a rounded box.
    pub const fn boxed(mut self) -> Self {
        self.boxed = true;
        self
    }

    fn line(&self) -> usize {
        match &self.target {
            AnnotationTarget::Point(position) => position.line,
            AnnotationTarget::Span(range) => range.start.line,
            AnnotationTarget::AnchoredSpan { range, .. } => range.start.line,
        }
    }
}

/// A source file and its annotations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source<'a> {
    text: &'a str,
    annotations: Vec<Annotation<'a>>,
    tab_width: usize,
}

impl<'a> Source<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            annotations: Vec::new(),
            tab_width: 4,
        }
    }

    pub fn annotation(mut self, annotation: Annotation<'a>) -> Self {
        self.annotations.push(annotation);
        self
    }

    pub fn tab_width(mut self, tab_width: usize) -> Self {
        self.tab_width = tab_width.max(1);
        self
    }

    pub fn render(&self) -> Result<String, RenderError> {
        let lines: Vec<_> = self.text.split('\n').collect();
        validate_annotations(&lines, &self.annotations)?;

        let line_number_width = lines.len().max(1).to_string().len();
        let gutter_padding = " ".repeat(line_number_width);
        let mut rendered = String::new();
        writeln!(rendered).unwrap();

        for (line_index, line) in lines.iter().enumerate() {
            let expanded = expand_tabs(line, self.tab_width);
            writeln!(
                rendered,
                "{:>line_number_width$} │ {}",
                line_index + 1,
                expanded.trim_end()
            )
            .unwrap();

            for annotation in self
                .annotations
                .iter()
                .filter(|annotation| annotation.line() == line_index)
            {
                let (start, marker, label_offset) =
                    annotation_marker(line, annotation, self.tab_width)?;
                let annotation_padding = " ".repeat(start);
                let label_padding = " ".repeat(label_offset);
                writeln!(rendered, "{gutter_padding} · {annotation_padding}{marker}").unwrap();
                if annotation.boxed {
                    render_boxed_label(
                        &mut rendered,
                        &gutter_padding,
                        &annotation_padding,
                        &label_padding,
                        annotation.label,
                    );
                    continue;
                }
                for (index, label_line) in annotation.label.lines().enumerate() {
                    if label_line.is_empty() {
                        writeln!(rendered, "{gutter_padding} ·").unwrap();
                        continue;
                    }
                    let connector = if index == 0 { "╰ " } else { "  " };
                    writeln!(
                        rendered,
                        "{gutter_padding} · {annotation_padding}{label_padding}{connector}{label_line}"
                    )
                    .unwrap();
                }
            }
        }

        Ok(rendered)
    }
}

fn render_boxed_label(
    rendered: &mut String,
    gutter_padding: &str,
    annotation_padding: &str,
    label_padding: &str,
    label: &str,
) {
    let lines = label.lines().collect::<Vec<_>>();
    let content_width = lines
        .iter()
        .map(|line| UnicodeWidthStr::width(*line))
        .max()
        .unwrap_or(0);
    let border = "─".repeat(content_width + 2);
    writeln!(
        rendered,
        "{gutter_padding} · {annotation_padding}{label_padding}│ ╭{border}╮"
    )
    .unwrap();
    for (index, line) in lines.into_iter().enumerate() {
        let padding = " ".repeat(content_width - UnicodeWidthStr::width(line));
        let connector = if index == 0 { "╰─┤" } else { "  │" };
        writeln!(
            rendered,
            "{gutter_padding} · {annotation_padding}{label_padding}{connector} {line}{padding} │"
        )
        .unwrap();
    }
    writeln!(
        rendered,
        "{gutter_padding} · {annotation_padding}{label_padding}  ╰{border}╯"
    )
    .unwrap();
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderError {
    LineOutOfBounds {
        line: usize,
    },
    CharacterOutOfBounds {
        position: Position,
    },
    InvalidByteOffset {
        offset: usize,
    },
    SpanRunsBackwards {
        start: Position,
        end: Position,
    },
    MultilineSpan {
        start: Position,
        end: Position,
    },
    AnchorOutsideSpan {
        range: Range<Position>,
        anchor: Position,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineOutOfBounds { line } => write!(f, "line {line} is out of bounds"),
            Self::CharacterOutOfBounds { position } => write!(
                f,
                "character {} on line {} is out of bounds",
                position.character, position.line
            ),
            Self::InvalidByteOffset { offset } => {
                write!(
                    f,
                    "byte offset {offset} is out of bounds or not a character boundary"
                )
            }
            Self::SpanRunsBackwards { start, end } => {
                write!(f, "span runs backwards from {start:?} to {end:?}")
            }
            Self::MultilineSpan { start, end } => {
                write!(f, "multiline spans are not supported: {start:?}..{end:?}")
            }
            Self::AnchorOutsideSpan { range, anchor } => {
                write!(f, "anchor {anchor:?} is outside span {range:?}")
            }
        }
    }
}

impl Error for RenderError {}

fn validate_annotations(lines: &[&str], annotations: &[Annotation<'_>]) -> Result<(), RenderError> {
    for annotation in annotations {
        match &annotation.target {
            AnnotationTarget::Point(position) => {
                validate_position(lines, *position)?;
            }
            AnnotationTarget::Span(range) => validate_span(lines, range)?,
            AnnotationTarget::AnchoredSpan { range, anchor } => {
                validate_span(lines, range)?;
                validate_position(lines, *anchor)?;
                if anchor.line != range.start.line
                    || anchor.character < range.start.character
                    || anchor.character > range.end.character
                {
                    return Err(RenderError::AnchorOutsideSpan {
                        range: range.clone(),
                        anchor: *anchor,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_span(lines: &[&str], range: &Range<Position>) -> Result<(), RenderError> {
    validate_position(lines, range.start)?;
    validate_position(lines, range.end)?;
    if range.start.line != range.end.line {
        return Err(RenderError::MultilineSpan {
            start: range.start,
            end: range.end,
        });
    }
    if range.start.character > range.end.character {
        return Err(RenderError::SpanRunsBackwards {
            start: range.start,
            end: range.end,
        });
    }
    Ok(())
}

fn validate_position(lines: &[&str], position: Position) -> Result<(), RenderError> {
    let line = lines
        .get(position.line)
        .ok_or(RenderError::LineOutOfBounds {
            line: position.line,
        })?;
    character_byte_offset(line, position).map(|_| ())
}

fn annotation_marker(
    line: &str,
    annotation: &Annotation<'_>,
    tab_width: usize,
) -> Result<(usize, String, usize), RenderError> {
    match &annotation.target {
        AnnotationTarget::Point(position) => {
            let byte = character_byte_offset(line, *position)?;
            Ok((display_width(&line[..byte], tab_width), "┬".into(), 0))
        }
        AnnotationTarget::Span(range) => {
            let start_byte = character_byte_offset(line, range.start)?;
            let end_byte = character_byte_offset(line, range.end)?;
            let start = display_width(&line[..start_byte], tab_width);
            let width = display_width(&line[start_byte..end_byte], tab_width).max(1);
            Ok((start, "─".repeat(width), 0))
        }
        AnnotationTarget::AnchoredSpan { range, anchor } => {
            let start_byte = character_byte_offset(line, range.start)?;
            let end_byte = character_byte_offset(line, range.end)?;
            let anchor_byte = character_byte_offset(line, *anchor)?;
            let start = display_width(&line[..start_byte], tab_width);
            let width = display_width(&line[start_byte..end_byte], tab_width).max(1);
            let anchor = display_width(&line[start_byte..anchor_byte], tab_width).min(width - 1);
            let marker = (0..width)
                .map(|column| if column == anchor { '┬' } else { '─' })
                .collect();
            Ok((start, marker, anchor))
        }
    }
}

fn character_byte_offset(line: &str, position: Position) -> Result<usize, RenderError> {
    if position.character == line.chars().count() {
        return Ok(line.len());
    }
    line.char_indices()
        .nth(position.character)
        .map(|(offset, _)| offset)
        .ok_or(RenderError::CharacterOutOfBounds { position })
}

fn display_width(text: &str, tab_width: usize) -> usize {
    expand_tabs(text, tab_width).width()
}

fn expand_tabs(text: &str, tab_width: usize) -> String {
    let mut expanded = String::new();
    let mut column = 0;
    for character in text.chars() {
        if character == '\t' {
            let spaces = tab_width - column % tab_width;
            expanded.push_str(&" ".repeat(spaces));
            column += spaces;
        } else {
            expanded.push(character);
            column += character.to_string().width();
        }
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_cursor_from_line_and_character() {
        let rendered = Source::new("from prod:requests\n| map avg")
            .annotation(Annotation::point(
                Position::new(1, 6),
                "first completion\nsecond completion",
            ))
            .render()
            .unwrap();

        assert_eq!(
            rendered,
            "\n1 │ from prod:requests\n2 │ | map avg\n  ·       ┬\n  ·       ╰ first completion\n  ·         second completion\n"
        );
    }

    #[test]
    fn renders_unicode_span_at_display_columns() {
        let rendered = Source::new("a😀 value")
            .annotation(Annotation::span(
                Position::new(0, 1)..Position::new(0, 2),
                "emoji",
            ))
            .render()
            .unwrap();

        assert_eq!(rendered, "\n1 │ a😀 value\n  ·  ──\n  ·  ╰ emoji\n");
    }

    #[test]
    fn renders_span_with_label_anchored_at_cursor() {
        let rendered = Source::new("map fill::const(0)")
            .annotation(Annotation::anchored_span(
                Position::new(0, 4)..Position::new(0, 15),
                Position::new(0, 10),
                "fill::const(value)\n\nFill missing values.",
            ))
            .render()
            .unwrap();

        assert_eq!(
            rendered,
            "\n1 │ map fill::const(0)\n  ·     ──────┬────\n  ·           ╰ fill::const(value)\n  ·\n  ·             Fill missing values.\n"
        );
    }

    #[test]
    fn renders_anchored_label_inside_a_rounded_box() {
        let rendered = Source::new("abcdef")
            .annotation(
                Annotation::anchored_span(
                    Position::new(0, 1)..Position::new(0, 5),
                    Position::new(0, 3),
                    "`item`\n\nhelp",
                )
                .boxed(),
            )
            .render()
            .unwrap();

        assert_eq!(
            rendered,
            "\n1 │ abcdef\n  ·  ──┬─\n  ·    │ ╭────────╮\n  ·    ╰─┤ `item` │\n  ·      │        │\n  ·      │ help   │\n  ·      ╰────────╯\n"
        );
    }

    #[test]
    fn converts_positions_and_utf8_byte_offsets() {
        let source = "a\n😀b";
        let position = Position::new(1, 1);
        assert_eq!(position.to_byte_offset(source), Ok(6));
        assert_eq!(Position::from_byte_offset(source, 6), Ok(position));
        assert!(Position::from_byte_offset(source, 3).is_err());
    }
}
