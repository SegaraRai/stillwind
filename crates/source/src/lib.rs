//! Source buffers, spans, line indexes, projections, and content fingerprints.
//!
//! The crate keeps two coordinate systems apart. Rust code addresses text in UTF-8
//! bytes ([`BytePos`], [`Span`]). Hosts and source maps address it in UTF-16 code units
//! ([`Position`], [`Utf16Offset`]). [`CoordinateIndex`] is the only supported
//! translation between them, and it rejects offsets inside a surrogate pair.
//!
//! [`SourceFile`] owns immutable text and validates every slice, so a [`Span`] is
//! meaningless outside the file that produced it. [`Fingerprint`] provides
//! process-independent content identity. The crate performs no I/O, and diagnostic
//! reporting belongs to its consumers.
//!
//! Modules:
//!
//! - [`coordinates`] converts between byte offsets and host coordinates.
//! - [`fingerprint`] provides content identity.
//! - [`identity`] holds opaque host keys.
//! - [`projection`] validates the correspondence from generated code to original
//!   source.
//! - [`text`] owns source buffers and spans.

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
mod identity;
mod projection;
mod text;

pub use coordinates::{CoordinateError, CoordinateIndex, Utf16Offset};
pub use fingerprint::Fingerprint;
pub use identity::{CssSourceName, ModuleId};
pub use projection::{
    ProjectionError, ProjectionIndex, ProjectionMap, SourceProvenance, VolarMapping,
};
pub use text::{
    BytePos, FileId, JoinText, Position, ReplaceText, SourceError, SourceFile, SourceRegistry,
    SourceRegistryError, SourceUri, SourceUriError, Span, Spanned,
};
