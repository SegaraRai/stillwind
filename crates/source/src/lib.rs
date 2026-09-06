//! Source buffers, spans, line indexes, and diagnostics used by every compiler pass.

use std::{ops::Range, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical project-relative module path used by generated identities and caches.
pub fn normalized_module_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("stillwind source files must be smaller than 4 GiB")
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FileId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BytePos(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub file: FileId,
    pub start: BytePos,
    pub end: BytePos,
}

impl Span {
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self {
            file,
            start: BytePos(start),
            end: BytePos(end),
        }
    }

    pub const fn len(self) -> u32 {
        self.end.0 - self.start.0
    }

    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub utf16_column: u32,
}

/// Immutable source text with byte-accurate spans and UTF-16 editor positions.
#[derive(Clone, Debug)]
pub struct SourceFile {
    id: FileId,
    path: Arc<str>,
    text: Arc<str>,
    line_starts: Arc<[u32]>,
}

impl SourceFile {
    pub fn new(id: FileId, path: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset(index + 1));
            }
        }

        Self {
            id,
            path: path.into(),
            text,
            line_starts: line_starts.into(),
        }
    }

    pub const fn id(&self) -> FileId {
        self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn full_span(&self) -> Span {
        Span::new(self.id, 0, offset(self.text.len()))
    }

    /// Returns the original source slice for `span`.
    ///
    /// # Errors
    ///
    /// Returns an error when the span belongs to another file, lies outside this
    /// source buffer, or is not aligned to UTF-8 character boundaries.
    pub fn slice(&self, span: Span) -> Result<&str, SourceError> {
        if span.file != self.id {
            return Err(SourceError::WrongFile {
                expected: self.id,
                actual: span.file,
            });
        }
        let range = span.start.0 as usize..span.end.0 as usize;
        self.text.get(range).ok_or(SourceError::InvalidSpan(span))
    }

    /// Converts a byte offset to the UTF-16 position expected by editor clients.
    ///
    /// # Errors
    ///
    /// Returns an error when the offset lies outside this source buffer or is
    /// not aligned to a UTF-8 character boundary.
    pub fn position(&self, byte_offset: BytePos) -> Result<Position, SourceError> {
        let byte_offset_usize = byte_offset.0 as usize;
        if byte_offset_usize > self.text.len() || !self.text.is_char_boundary(byte_offset_usize) {
            return Err(SourceError::InvalidOffset(byte_offset));
        }

        let line_index = self
            .line_starts
            .partition_point(|start| *start <= byte_offset.0)
            - 1;
        let line_start = self.line_starts[line_index] as usize;
        let utf16_column = offset(
            self.text[line_start..byte_offset_usize]
                .encode_utf16()
                .count(),
        );

        Ok(Position {
            line: offset(line_index),
            utf16_column,
        })
    }

    /// Converts a UTF-8 byte offset to a document-wide UTF-16 offset.
    ///
    /// `CodeMirror` and `Monaco` use this coordinate system for selections.
    ///
    /// # Errors
    ///
    /// Returns an error when the offset lies outside this source buffer or is
    /// not aligned to a UTF-8 character boundary.
    pub fn utf16_offset(&self, byte_offset: BytePos) -> Result<u32, SourceError> {
        let byte_offset_usize = byte_offset.0 as usize;
        if byte_offset_usize > self.text.len() || !self.text.is_char_boundary(byte_offset_usize) {
            return Err(SourceError::InvalidOffset(byte_offset));
        }
        Ok(offset(
            self.text[..byte_offset_usize].encode_utf16().count(),
        ))
    }
}

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("span belongs to file {actual:?}, expected {expected:?}")]
    WrongFile { expected: FileId, actual: FileId },
    #[error("invalid source span {0:?}")]
    InvalidSpan(Span),
    #[error("invalid byte offset {0:?}")]
    InvalidOffset(BytePos),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileError {
    pub code: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl CompileError {
    pub fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            message: message.into(),
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LintFinding {
    pub rule: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl LintFinding {
    pub fn new(rule: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            rule,
            message: message.into(),
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LintDirectiveKind {
    Disable,
    Enable,
    DisableLine,
    DisableNextLine,
}

#[derive(Clone, Debug)]
struct LintDirective {
    kind: LintDirectiveKind,
    offset: usize,
    line: usize,
    rules: Vec<String>,
}

/// Removes lint findings suppressed by `swlint-disable*` comments.
///
/// The source span is deliberately used after all analysis has completed, so
/// findings produced by different compiler passes share the same suppression
/// semantics. Both line and block comments are accepted.
pub fn filter_lint_findings(source: &SourceFile, findings: &mut Vec<LintFinding>) {
    let directives = collect_lint_directives(source.text());
    if directives.is_empty() {
        return;
    }
    let line_starts = source
        .text()
        .match_indices('\n')
        .map(|(offset, _)| offset + 1)
        .chain(std::iter::once(source.text().len()))
        .collect::<Vec<_>>();
    findings.retain(|finding| {
        let span = finding
            .labels
            .first()
            .map_or(source.full_span(), |label| label.span);
        let finding_offset = span.start.0 as usize;
        let finding_line = line_starts.partition_point(|start| *start <= finding_offset);
        let mut disabled = std::collections::BTreeSet::new();
        let mut disabled_all = false;
        let mut suppressed = false;
        for directive in &directives {
            let matches_rule = |rules: &[String]| {
                rules.is_empty() || rules.iter().any(|rule| rule == "*" || rule == finding.rule)
            };
            match directive.kind {
                LintDirectiveKind::DisableLine if directive.line == finding_line => {
                    suppressed |= matches_rule(&directive.rules);
                }
                LintDirectiveKind::DisableNextLine if directive.line + 1 == finding_line => {
                    suppressed |= matches_rule(&directive.rules);
                }
                LintDirectiveKind::Disable if directive.offset <= finding_offset => {
                    if matches_rule(&directive.rules) {
                        if directive.rules.is_empty()
                            || directive.rules.iter().any(|rule| rule == "*")
                        {
                            disabled_all = true;
                        } else {
                            disabled.extend(directive.rules.iter().cloned());
                        }
                    }
                }
                LintDirectiveKind::Enable if directive.offset <= finding_offset => {
                    if directive.rules.is_empty() || directive.rules.iter().any(|rule| rule == "*")
                    {
                        disabled_all = false;
                        disabled.clear();
                    } else {
                        for rule in &directive.rules {
                            disabled.remove(rule);
                        }
                    }
                }
                _ => {}
            }
        }
        !(suppressed || disabled_all || disabled.contains(finding.rule))
    });
}

fn collect_lint_directives(source: &str) -> Vec<LintDirective> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut cursor = 0;
    let mut quote = None;
    while cursor + 1 < bytes.len() {
        if let Some(delimiter) = quote {
            if bytes[cursor] == b'\\' {
                cursor = (cursor + 2).min(bytes.len());
            } else {
                if bytes[cursor] == delimiter {
                    quote = None;
                }
                cursor += 1;
            }
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"' | b'`') {
            quote = Some(bytes[cursor]);
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'/' && bytes[cursor + 1] == b'/' {
            let start = cursor + 2;
            cursor = source[cursor..]
                .find('\n')
                .map_or(bytes.len(), |offset| cursor + offset);
            comments.push((start, &source[start..cursor]));
        } else if bytes[cursor] == b'/' && bytes[cursor + 1] == b'*' {
            let start = cursor + 2;
            cursor = source[cursor + 2..]
                .find("*/")
                .map_or(bytes.len(), |offset| cursor + 2 + offset);
            comments.push((start, &source[start..cursor]));
            cursor = (cursor + 2).min(bytes.len());
        } else {
            cursor += 1;
        }
    }
    comments
        .into_iter()
        .filter_map(|(offset, body)| {
            let relative = body.find("swlint-")?;
            let directive_offset = offset + relative;
            let rest = &body[relative + "swlint-".len()..];
            let (kind, rest) = [
                ("disable-next-line", LintDirectiveKind::DisableNextLine),
                ("disable-line", LintDirectiveKind::DisableLine),
                ("disable", LintDirectiveKind::Disable),
                ("enable", LintDirectiveKind::Enable),
            ]
            .into_iter()
            .find_map(|(name, kind)| rest.strip_prefix(name).map(|rest| (kind, rest)))?;
            let rules = rest
                .split(|character: char| character.is_whitespace() || character == ',')
                .filter(|rule| !rule.is_empty())
                .map(str::to_owned)
                .collect();
            let line = source[..directive_offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            Some(LintDirective {
                kind,
                offset: directive_offset,
                line,
                rules,
            })
        })
        .collect()
}

pub fn range_of(span: Span) -> Range<usize> {
    span.start.0 as usize..span.end.0 as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_byte_offsets_to_utf16_positions() {
        let source = SourceFile::new(FileId(1), "Button.tsx", "a😀\nβ");

        assert_eq!(
            source.position(BytePos(0)).unwrap(),
            Position {
                line: 0,
                utf16_column: 0
            }
        );
        assert_eq!(
            source.position(BytePos(5)).unwrap(),
            Position {
                line: 0,
                utf16_column: 3
            }
        );
        assert_eq!(
            source.position(BytePos(6)).unwrap(),
            Position {
                line: 1,
                utf16_column: 0
            }
        );
        assert_eq!(source.utf16_offset(BytePos(6)).unwrap(), 4);
    }

    #[test]
    fn normalizes_module_paths_across_platforms() {
        assert_eq!(
            normalized_module_path(r".\src\components\Menu.tsx"),
            "src/components/Menu.tsx"
        );
        assert_eq!(
            normalized_module_path("./src/components/Menu.tsx"),
            "src/components/Menu.tsx"
        );
    }
}
