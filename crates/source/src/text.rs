//! Byte-accurate source text, spans, and editor coordinates.
//!
//! [`SourceFile`] owns immutable text together with its line table and is the only
//! authority that can validate a slice: spans are half-open UTF-8 byte ranges tagged
//! with a [`FileId`], and a span is meaningless outside the file it was created for.
//! Byte offsets are the internal currency; [`SourceFile::position`] and
//! [`SourceFile::utf16_offset`] exist only to talk to editors, which count UTF-16 code
//! units.
//!
//! [`normalized_module_path`] is the canonical spelling used in generated identities
//! and cache keys. Identity-producing passes must normalize through it rather than
//! local `Path` handling, so one module keeps one name on Windows and POSIX. Nothing
//! in this module performs I/O.

use std::ops::Range;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Failure to build a source, or to relate a span or offset to one.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceError {
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

/// Canonical project-relative module path used by generated identities and caches.
///
/// Backslashes become `/` and leading `./` segments are stripped; nothing else changes,
/// so no segment is resolved, collapsed or re-cased. The function is idempotent, which
/// lets any pass normalize a path it received without disturbing one that was already
/// normalized.
///
/// The input must be project-relative. The result is not a real path: `..` segments and
/// absolute prefixes survive verbatim, so this must not be used to compare two
/// spellings of the same file.
pub fn normalized_module_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

/// Narrows a [`usize`] length or index into the crate's 32-bit coordinate space.
///
/// Every conversion in this module goes through here so an oversized source fails with
/// one error instead of being truncated somewhere else.
pub(super) fn offset(value: usize) -> Result<u32, SourceError> {
    u32::try_from(value).map_err(|_| SourceError::TooLarge(value))
}

/// Converts a span to a [`Range<usize>`] for indexing or slicing.
///
/// No validation happens here: the caller must already know the range is in bounds.
/// [`SourceFile::slice`] is the checked alternative.
pub fn range_of(span: Span) -> Range<usize> {
    span.start.0 as usize..span.end.0 as usize
}

/// Process-local identifier of a source buffer.
///
/// Ids are assigned per run and are not stable across processes, so they must never
/// be used alone as durable identity. In-process artifact keys may include them to
/// distinguish source buffers; persisted records need a durable source name. Anything
/// that needs a durable name should use [`normalized_module_path`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FileId(
    /// Process-local numeric buffer identity; not a persistent file identifier.
    pub u32,
);

/// A UTF-8 byte offset from the start of one [`SourceFile`].
///
/// The value carries no file identity; it is only meaningful together with the
/// `FileId` of the span it came from.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BytePos(
    /// Zero-based UTF-8 byte count; construction does not validate bounds or scalar alignment.
    pub u32,
);

/// A half-open byte range `[start, end)` in one source file.
///
/// `start == end` is a valid empty span that addresses a position rather than a
/// character. Construction validates nothing, so a span may be reversed or out of
/// range; [`SourceFile::slice`] validates the whole span. [`SourceFile::position`]
/// validates only the byte offset supplied to it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Span {
    /// Buffer the offsets belong to.
    pub file: FileId,
    /// Inclusive start offset.
    pub start: BytePos,
    /// Exclusive end offset.
    pub end: BytePos,
}

impl Span {
    /// Builds a span from raw offsets without checking them; `end` may precede `start`.
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self {
            file,
            start: BytePos(start),
            end: BytePos(end),
        }
    }

    /// Returns `None` for a reversed span.
    ///
    /// Callers must decide what `None` means for them; treating it as zero silently
    /// accepts a malformed span.
    pub const fn len(self) -> Option<u32> {
        self.end.0.checked_sub(self.start.0)
    }

    /// True when the span covers no bytes.
    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }
}

/// A value together with the source range it was parsed from.
///
/// Derived equality compares both the value and the span. Equal values from different
/// source ranges therefore compare unequal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Spanned<T> {
    /// The parsed value.
    pub value: T,
    /// Where in its source the value was found.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Pairs a value with its source range without validating the range.
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// A zero-based line and UTF-16 column, the coordinate editors and LSP use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Zero-based line index within the file.
    pub line: u32,
    /// Zero-based UTF-16 code-unit column within that line.
    pub utf16_column: u32,
}

/// Immutable source text with byte-accurate spans and UTF-16 editor positions.
///
/// Cloning shares the source storage and its coordinate index.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Process-local buffer identity carried by spans.
    id: FileId,
    /// Host-supplied path stored verbatim for display and module identity derivation;
    /// never opened here.
    path: Arc<str>,
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
    /// The path is stored verbatim and only used for display and for deriving
    /// [`normalized_module_path`]; it is never opened.
    ///
    /// # Errors
    ///
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

    /// The identity this buffer was registered under. Spans created from this file
    /// carry the same id.
    pub const fn id(&self) -> FileId {
        self.id
    }

    /// Host-supplied display path, unchanged by this crate.
    pub fn path(&self) -> &str {
        &self.path
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A spanned value compares its range as well as its value.
    #[test]
    fn spanned_equality_includes_the_source_range() {
        let a = Spanned::new("same", Span::new(FileId(1), 0, 4));
        let b = Spanned::new("same", Span::new(FileId(1), 5, 9));
        assert_ne!(a, b);
        assert_eq!(a.value, b.value);
    }

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

        #[test]
        fn span_lengths_and_ranges_agree(start in 0_u32..1000, len in 0_u32..1000) {
            let span = Span::new(FileId(1), start, start + len);
            proptest::prop_assert_eq!(span.len(), Some(len));
            proptest::prop_assert_eq!(span.is_empty(), len == 0);
            proptest::prop_assert_eq!(range_of(span).len(), usize::try_from(len).unwrap());
        }
    }
}
