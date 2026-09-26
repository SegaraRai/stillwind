//! Source buffers, spans, line indexes, projections, and content fingerprints.
//!
//! Two coordinate systems are kept apart. Rust passes address text in UTF-8 bytes
//! ([`BytePos`], [`Span`]); hosts and source maps address it in UTF-16 code units
//! ([`Position`], [`Utf16Offset`]). [`CoordinateIndex`] is the only sanctioned
//! translation between them, and it rejects offsets inside a surrogate pair.
//!
//! [`SourceFile`] owns immutable text and validates every slice, so a [`Span`] is
//! meaningless outside the file that produced it. [`Fingerprint`] provides
//! process-independent content identity. The crate performs no I/O.
//!
//! `text` owns source buffers and spans; `coordinates` converts host coordinates;
//! `projection` validates generated-to-original correspondence; `fingerprint` provides
//! content and normalized module identities. Diagnostic reporting belongs to consumers.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod coordinates;
mod fingerprint;
mod projection;
mod text;

pub use coordinates::{CoordinateError, CoordinateIndex, Utf16Offset};
pub use fingerprint::Fingerprint;
pub use projection::{
    ProjectionError, ProjectionIndex, ProjectionMap, SourceProvenance, VolarMapping,
};
pub use text::{
    BytePos, FileId, Position, SourceError, SourceFile, SourceRegistry, SourceRegistryError,
    SourceUri, SourceUriError, Span, Spanned, normalized_module_path, range_of,
};
