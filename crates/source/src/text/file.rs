//! Immutable UTF-8 text, checked slices, and UTF-16 editor coordinates.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{BytePos, FileId, SourceUri, Span};

/// A zero-based line and UTF-16 column, the coordinate editors and LSP use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Zero-based line index within the file.
    pub line: u32,
    /// Zero-based UTF-16 code-unit column within that line.
    pub utf16_column: u32,
}

/// Failure to build a source, or to relate a span or offset to one.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceError {
    /// An internal buffer cannot be named as an original source-map document.
    #[error("an internal source cannot have a source-map entry")]
    InternalSourceMap,
    /// A length or offset does not fit the 32-bit coordinate space.
    #[error("source size or offset {0} exceeds the 32-bit source limit")]
    TooLarge(
        /// Requested length or offset exceeding [`u32::MAX`].
        usize,
    ),
    /// A byte range is reversed or extends past its text.
    #[error("invalid source byte range {start}..{end}")]
    InvalidRange {
        /// Requested inclusive UTF-8 byte offset.
        start: usize,
        /// Requested exclusive UTF-8 byte offset.
        end: usize,
    },
    /// The span was created for a different source buffer.
    #[error("span belongs to file {actual:?}, expected {expected:?}")]
    WrongFile {
        /// File the span should have belonged to.
        expected: FileId,
        /// File the span actually names.
        actual: FileId,
    },
    /// The span is reversed, out of range, or not on character boundaries.
    #[error("invalid source span {0:?}")]
    InvalidSpan(
        /// Rejected half-open UTF-8 span, including its buffer identity.
        Span,
    ),
    /// The byte offset is out of range or splits a character.
    #[error("invalid byte offset {0:?}")]
    InvalidOffset(
        /// Rejected UTF-8 byte offset.
        BytePos,
    ),
}

/// Immutable source text with byte-accurate spans and UTF-16 editor positions.
///
/// Cloning shares the source storage and its coordinate index.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Buffer identity carried by spans within its source scope.
    id: FileId,
    /// Host-supplied document identity, present for user-facing source reports.
    uri: Option<SourceUri>,
    /// Exact source entry the host selected for generated CSS maps.
    source_map_source: Option<Arc<str>>,
    /// Immutable UTF-8 source buffer whose byte length fits u32.
    text: Arc<str>,
    /// Ascending UTF-8 byte offsets starting with zero, followed by the position after
    /// each LF.
    line_starts: Arc<[u32]>,
    /// Cached UTF-8 byte length of text, also the valid EOF offset.
    len: u32,
}

impl SourceFile {
    /// Builds a buffer and its line table in one pass over the text.
    ///
    /// A missing URI identifies an internal buffer that cannot be reported directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the text exceeds the 32-bit source coordinate limit.
    pub fn new(
        id: FileId,
        uri: Option<SourceUri>,
        source_map_source: Option<Arc<str>>,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, SourceError> {
        if uri.is_none() && source_map_source.is_some() {
            return Err(SourceError::InternalSourceMap);
        }
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
            uri,
            source_map_source,
            text,
            line_starts: line_starts.into(),
            len,
        })
    }

    /// Returns the document identity, when the host supplied one.
    pub fn uri(&self) -> Option<&SourceUri> {
        self.uri.as_ref()
    }

    /// Returns the host's source-map entry for this original buffer.
    pub fn source_map_source(&self) -> Option<&str> {
        self.source_map_source.as_deref()
    }

    /// The identity this buffer was registered under. Spans created from this file
    /// carry the same id.
    pub const fn id(&self) -> FileId {
        self.id
    }

    /// The complete text. Prefer `slice` when a span is involved, so the range is
    /// validated.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Span covering the whole text, used when a diagnostic has no better location to
    /// point at.
    pub fn full_span(&self) -> Span {
        Span::new(self.id, 0, self.len)
    }

    /// Returns the original source slice for `span`.
    ///
    /// The returned text is a borrow of this buffer, not a copy, and is the exact
    /// original text: callers must not reconstruct it from anything else.
    ///
    /// # Errors
    ///
    /// Returns an error when the span belongs to another file, lies outside this source
    /// buffer, or is not aligned to UTF-8 character boundaries.
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
    /// Columns count UTF-16 code units from the start of the line. Only UTF-8 scalar
    /// boundaries are accepted; supplementary characters advance the column by two.
    ///
    /// # Errors
    ///
    /// Returns an error when the offset lies outside this source buffer or is not
    /// aligned to a UTF-8 character boundary.
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
    /// Returns an error when the offset lies outside this source buffer or is not
    /// aligned to a UTF-8 character boundary.
    pub fn utf16_offset(&self, byte_offset: BytePos) -> Result<u32, SourceError> {
        let byte_offset_usize = byte_offset.0 as usize;
        if byte_offset_usize > self.text.len() || !self.text.is_char_boundary(byte_offset_usize) {
            return Err(SourceError::InvalidOffset(byte_offset));
        }
        offset(self.text[..byte_offset_usize].encode_utf16().count())
    }
}

/// Narrows a [`usize`] length or index into the crate's 32-bit coordinate space.
///
/// Every conversion in this module goes through here so an oversized source fails with
/// one error instead of being truncated somewhere else.
fn offset(value: usize) -> Result<u32, SourceError> {
    u32::try_from(value).map_err(|_| SourceError::TooLarge(value))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn maps_byte_offsets_to_utf16_positions() {
        let source = SourceFile::new(FileId(1), None, None, "a😀\nβ").unwrap();

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
    fn invalid_byte_boundaries_and_wrong_files_are_rejected() {
        let source = SourceFile::new(FileId(1), None, None, "a😀\r\nz").unwrap();
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
}

#[cfg(test)]
mod property_tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn utf16_offsets_agree_with_character_boundaries(chars in proptest::collection::vec(proptest::char::any(), 0..60)) {
            let text: String = chars.into_iter().collect();
            let source = SourceFile::new(FileId(1), None, None, text.clone()).unwrap();
            for offset in text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())) {
                proptest::prop_assert_eq!(
                    source.utf16_offset(BytePos(u32::try_from(offset).unwrap())).unwrap(),
                    u32::try_from(text[..offset].encode_utf16().count()).unwrap()
                );
            }
        }

    }
}
