//! Byte-accurate source text, spans, and editor coordinates.
//!
//! [`SourceFile`] owns immutable text together with its line table and is the only
//! authority that can validate a slice: spans are half-open UTF-8 byte ranges tagged
//! with a [`FileId`], and a span is meaningless outside the file it was created for.
//! Byte offsets are the internal currency; [`SourceFile::position`] and
//! [`SourceFile::utf16_offset`] exist only to talk to editors, which count UTF-16 code
//! units.

//!
//! `uri` validates host document identities. `span` defines byte ranges and their
//! buffer identities. `file` validates immutable text and converts editor positions.
//! `registry` retains revisions and allocates IDs within one project scope.

mod file;
mod registry;
mod span;
mod uri;

pub use file::{Position, SourceError, SourceFile};
pub use registry::{SourceRegistry, SourceRegistryError};
pub use span::{BytePos, FileId, Span, Spanned, range_of};
pub use uri::{SourceUri, SourceUriError};
