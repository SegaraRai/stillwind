//! Byte-accurate source text, spans, and editor coordinates.
//!
//! [`SourceFile`] owns immutable text together with its line table and is the only
//! authority that can validate a slice: spans are half-open UTF-8 byte ranges tagged
//! with a [`FileId`], and a span is meaningless outside the file it was created for.
//! Byte offsets are the internal currency; [`SourceFile::position`] and
//! [`SourceFile::utf16_offset`] exist only to talk to editors, which count UTF-16 code
//! units.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

/// An invalid source-document identifier supplied by a host.
#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid source URI: {0}")]
pub struct SourceUriError(pub String);

/// Host-supplied identity of an editable or virtual source document.
///
/// The spelling is preserved because a host or editor may distinguish valid URI
/// spellings that a generic normalizer would consider equivalent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SourceUri(LeanStr);

impl SourceUri {
    /// Validates an absolute URI without changing its spelling.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing scheme or malformed URI characters.
    pub fn parse(value: impl Into<LeanStr>) -> Result<Self, SourceUriError> {
        let value = value.into();
        let Some((scheme, rest)) = value.split_once(':') else {
            return Err(SourceUriError(value.to_string()));
        };
        let valid_scheme = !scheme.is_empty()
            && scheme.bytes().enumerate().all(|(index, byte)| {
                if index == 0 {
                    byte.is_ascii_alphabetic()
                } else {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
                }
            });
        let bytes = rest.as_bytes();
        let mut index = 0;
        let mut valid_rest = !rest.is_empty();
        while index < bytes.len() {
            let byte = bytes[index];
            if byte == b'%' {
                valid_rest &= bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
                    && bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit);
                index += 3;
            } else {
                valid_rest &=
                    byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=".contains(&byte);
                index += 1;
            }
        }
        if !valid_scheme || !valid_rest {
            return Err(SourceUriError(value.to_string()));
        }
        Ok(Self(value))
    }

    /// Returns the exact identifier supplied by the host.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SourceUri {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Converts a span to a [`Range<usize>`] for indexing or slicing.
///
/// No validation happens here: the caller must already know the range is in bounds.
/// [`SourceFile::slice`] is the checked alternative.
pub fn range_of(span: Span) -> Range<usize> {
    span.start.0 as usize..span.end.0 as usize
}

/// Identifier of a source buffer within one project source scope.
///
/// IDs may repeat in independent scopes and are not stable across processes.
/// Persisted records need a durable document or module identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FileId(
    /// Buffer sequence within its source scope; not a persistent file identifier.
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
pub(super) fn offset(value: usize) -> Result<u32, SourceError> {
    u32::try_from(value).map_err(|_| SourceError::TooLarge(value))
}

/// Failure to register or resolve a project source buffer.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceRegistryError {
    /// No further IDs can be issued in this source scope.
    #[error("source identifier space is exhausted")]
    FileIdExhausted,
    /// A span names a buffer absent from this registry.
    #[error("unknown source file {0:?}")]
    UnknownFile(FileId),
    /// An ID already belongs to a different immutable source revision.
    #[error("source file ID {0:?} belongs to another revision")]
    ConflictingFile(FileId),
    /// A generated buffer has no original document to report.
    #[error("source file {0:?} has no reportable document URI")]
    UnreportableSource(FileId),
    /// The registered text or requested range is invalid.
    #[error("{0}")]
    Source(#[from] SourceError),
}

/// Source buffers and the ID allocator shared by one project session and its loaders.
#[derive(Clone, Debug)]
pub struct SourceRegistry {
    files: BTreeMap<FileId, Arc<SourceFile>>,
    next_id: Arc<AtomicU32>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            next_id: Arc::new(AtomicU32::new(1 << 31)),
        }
    }
}

impl SourceRegistry {
    fn next_id(&self) -> Result<FileId, SourceRegistryError> {
        let id = self
            .next_id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1).filter(|id| *id < u32::MAX)
            })
            .map_err(|_| SourceRegistryError::FileIdExhausted)?;
        Ok(FileId(id))
    }

    /// Registers one immutable source revision and issues its compilation file ID.
    ///
    /// `None` for `uri` identifies an internal buffer whose positions cannot be
    /// published without projection to an original document.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is too large or the ID space is exhausted.
    pub fn register(
        &mut self,
        uri: Option<SourceUri>,
        source_map_source: Option<Arc<str>>,
        text: impl Into<Arc<str>>,
    ) -> Result<Arc<SourceFile>, SourceRegistryError> {
        let id = self.next_id()?;
        let source = Arc::new(SourceFile::new(id, uri, source_map_source, text)?);
        self.files.insert(source.id(), source.clone());
        Ok(source)
    }

    /// Issues an ID for a generated analysis buffer that will be built after projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID space is exhausted.
    pub fn reserve_generated_id(&mut self) -> Result<FileId, SourceRegistryError> {
        self.next_id()
    }

    /// Starts an empty registry that issues IDs from this registry's project scope.
    #[must_use]
    pub fn fork_empty(&self) -> Self {
        Self {
            files: BTreeMap::new(),
            next_id: self.next_id.clone(),
        }
    }

    /// Whether two registries issue IDs from the same project scope.
    pub fn shares_ids_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.next_id, &other.next_id)
    }

    /// Keeps later allocations above an already issued source ID in this scope.
    pub fn reserve_after(&self, id: FileId) {
        self.next_id
            .fetch_max(id.0.saturating_add(1), Ordering::Relaxed);
    }

    /// Finds the registered source that issued a span's file ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID was not issued by this registry.
    pub fn source(&self, id: FileId) -> Result<&Arc<SourceFile>, SourceRegistryError> {
        self.files
            .get(&id)
            .ok_or(SourceRegistryError::UnknownFile(id))
    }

    /// Returns the source with the smallest issued ID, if one exists.
    pub fn first(&self) -> Option<&Arc<SourceFile>> {
        self.files.values().next()
    }

    /// Retains an already registered source revision from another project component.
    ///
    /// The source carries its ID, URI, text, and coordinate table together; a
    /// conflicting ID is rejected instead of replacing an existing revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID already belongs to a different revision.
    pub fn retain(&mut self, source: Arc<SourceFile>) -> Result<(), SourceRegistryError> {
        if let Some(existing) = self.files.get(&source.id())
            && !Arc::ptr_eq(existing, &source)
        {
            return Err(SourceRegistryError::ConflictingFile(source.id()));
        }
        self.files.insert(source.id(), source);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids_are_shared_only_with_explicitly_forked_registries() {
        let mut first = SourceRegistry::default();
        let mut reload = first.fork_empty();
        let original = first
            .register(Some(SourceUri::parse("untitled:one").unwrap()), None, "a")
            .unwrap();
        let revised = reload
            .register(Some(SourceUri::parse("untitled:one").unwrap()), None, "b")
            .unwrap();
        assert_ne!(original.id(), revised.id());
        assert!(first.shares_ids_with(&reload));
        let mut separate = SourceRegistry::default();
        assert!(!first.shares_ids_with(&separate));
        let foreign = separate
            .register(Some(SourceUri::parse("untitled:other").unwrap()), None, "c")
            .unwrap();
        assert_eq!(original.id(), foreign.id());
        assert_eq!(revised.uri().unwrap().as_str(), "untitled:one");
        assert_eq!(revised.text(), "b");
    }

    #[test]
    fn source_uris_preserve_host_spelling_and_reject_paths() {
        for value in [
            "file:///F:/Project/a%20b.tsx",
            "untitled:1",
            "https://example.test/app.tsx",
        ] {
            let uri = SourceUri::parse(value).unwrap();
            assert_eq!(uri.as_str(), value);
            let restored: SourceUri =
                serde_json::from_str(&serde_json::to_string(&uri).unwrap()).unwrap();
            assert_eq!(restored, uri);
        }
        for value in [
            r"F:\Project\App.tsx",
            "src/App.tsx",
            ":bad",
            "file:///bad%2",
            "file:///has space",
        ] {
            assert!(SourceUri::parse(value).is_err(), "{value}");
        }
    }

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
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

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

        #[test]
        fn span_lengths_and_ranges_agree(start in 0_u32..1000, len in 0_u32..1000) {
            let span = Span::new(FileId(1), start, start + len);
            proptest::prop_assert_eq!(span.len(), Some(len));
            proptest::prop_assert_eq!(span.is_empty(), len == 0);
            proptest::prop_assert_eq!(range_of(span).len(), usize::try_from(len).unwrap());
        }
    }
}
