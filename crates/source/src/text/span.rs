//! Unchecked byte coordinates tagged with a source buffer identity.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Converts a span to a [`Range<usize>`] for indexing or slicing.
///
/// No validation happens here: the caller must already know the range is in bounds.
/// [`super::SourceFile::slice`] is the checked alternative.
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

/// A UTF-8 byte offset from the start of one [`super::SourceFile`].
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
/// range; [`super::SourceFile::slice`] validates the whole span. [`super::SourceFile::position`]
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
}

#[cfg(test)]
mod property_tests {
    use super::*;

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
