//! File-local UTF-8 range keys for frontend bookkeeping. Coordinate mapping and file
//! identity remain the responsibility of the collection that owns the keys.

use oxc::span::Span as OxcSpan;

/// A half-open UTF-8 byte range within one source buffer. Collections must use one
/// buffer and coordinate space; comparing ranges from original and projected buffers
/// requires mapping them into the same space first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct LocalSpan {
    /// Inclusive byte offset.
    pub(crate) start: u32,
    /// Exclusive byte offset.
    pub(crate) end: u32,
}

impl LocalSpan {
    /// Records `start..end` without validation. The caller must supply ordered UTF-8
    /// boundaries within the source buffer used by the destination collection.
    pub(crate) const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

impl From<OxcSpan> for LocalSpan {
    /// Preserves the AST span's offsets and coordinate space without validation.
    fn from(span: OxcSpan) -> Self {
        Self::new(span.start, span.end)
    }
}
