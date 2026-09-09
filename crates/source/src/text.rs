use std::ops::Range;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source size or offset {0} exceeds the 32-bit source limit")]
    TooLarge(usize),
    #[error("invalid source byte range {start}..{end}")]
    InvalidRange { start: usize, end: usize },
    #[error("span belongs to file {actual:?}, expected {expected:?}")]
    WrongFile { expected: FileId, actual: FileId },
    #[error("invalid source span {0:?}")]
    InvalidSpan(Span),
    #[error("invalid byte offset {0:?}")]
    InvalidOffset(BytePos),
}

/// Canonical project-relative module path used by generated identities and caches.
pub fn normalized_module_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

pub(super) fn offset(value: usize) -> Result<u32, SourceError> {
    u32::try_from(value).map_err(|_| SourceError::TooLarge(value))
}

pub fn range_of(span: Span) -> Range<usize> {
    span.start.0 as usize..span.end.0 as usize
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

    /// Returns `None` for a reversed span.
    pub const fn len(self) -> Option<u32> {
        self.end.0.checked_sub(self.start.0)
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
    len: u32,
}

impl SourceFile {
    /// # Errors
    /// Returns an error if the text exceeds the 32-bit source coordinate limit.
    pub fn new(
        id: FileId,
        path: impl Into<Arc<str>>,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, SourceError> {
        let text = text.into();
        let len = offset(text.len())?;
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset(index + 1)?);
            }
        }

        Ok(Self {
            id,
            path: path.into(),
            text,
            line_starts: line_starts.into(),
            len,
        })
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
        Span::new(self.id, 0, self.len)
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
        )?;

        Ok(Position {
            line: offset(line_index)?,
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
        offset(self.text[..byte_offset_usize].encode_utf16().count())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reversed_spans_and_oversized_offsets_are_checked() {
        assert_eq!(Span::new(FileId(1), 2, 1).len(), None);
        assert_eq!(Span::new(FileId(1), 0, u32::MAX).len(), Some(u32::MAX));
        assert_eq!(offset(u32::MAX as usize).unwrap(), u32::MAX);
        if let Some(too_large) = (u32::MAX as usize).checked_add(1) {
            assert!(
                matches!(offset(too_large), Err(SourceError::TooLarge(value)) if value == too_large)
            );
        }
    }

    use super::*;

    #[test]
    fn maps_byte_offsets_to_utf16_positions() {
        let source = SourceFile::new(FileId(1), "Button.tsx", "a😀\nβ").unwrap();

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

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn invalid_byte_boundaries_and_wrong_files_are_rejected() {
        let source = SourceFile::new(FileId(1), "x", "a😀\r\nz").unwrap();
        assert!(matches!(
            source.slice(Span::new(FileId(2), 0, 1)),
            Err(SourceError::WrongFile { .. })
        ));
        assert!(source.slice(Span::new(FileId(1), 2, 5)).is_err());
        assert!(source.position(BytePos(2)).is_err());
        assert_eq!(source.utf16_offset(BytePos(5)).unwrap(), 3);
        assert_eq!(
            source.position(BytePos(7)).unwrap(),
            Position {
                line: 1,
                utf16_column: 0
            }
        );
        assert_eq!(source.slice(Span::new(FileId(1), 1, 5)).unwrap(), "😀");
    }

    proptest::proptest! {
        #[test]
        fn utf16_offsets_agree_with_character_boundaries(chars in proptest::collection::vec(proptest::char::any(), 0..60)) {
            let text: String = chars.into_iter().collect();
            let source = SourceFile::new(FileId(1), "x", text.clone()).unwrap();
            for offset in text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())) {
                proptest::prop_assert_eq!(
                    source.utf16_offset(BytePos(u32::try_from(offset).unwrap())).unwrap(),
                    u32::try_from(text[..offset].encode_utf16().count()).unwrap()
                );
            }
        }

        #[test]
        fn path_normalization_is_idempotent(path in "[a-zA-Z0-9./\\\\_-]{0,60}") {
            let normalized = normalized_module_path(&path);
            proptest::prop_assert_eq!(normalized_module_path(&normalized), normalized);
        }
    }

    proptest::proptest! {
        #[test]
        fn span_lengths_and_ranges_agree(start in 0_u32..1000, len in 0_u32..1000) {
            let span = Span::new(FileId(1), start, start + len);
            proptest::prop_assert_eq!(span.len(), Some(len));
            proptest::prop_assert_eq!(span.is_empty(), len == 0);
            proptest::prop_assert_eq!(range_of(span).len(), usize::try_from(len).unwrap());
        }
    }
}
