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
