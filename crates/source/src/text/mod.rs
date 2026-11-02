//! Byte-accurate source text, spans, and editor coordinates.
//!
//! [`SourceFile`] owns immutable text with its line table and is the only authority
//! that can validate a slice. Spans are half-open UTF-8 byte ranges tagged with a
//! [`FileId`], and a span is meaningless outside the file that it was created for. Byte
//! offsets are the internal unit. [`SourceFile::position`] and
//! [`SourceFile::utf16_offset`] exist only for editors, which count UTF-16 code units.
//!
//! Modules:
//!
//! - [`construction`] joins and replaces compact owned text.
//! - [`mod@file`] validates immutable text and converts editor positions.
//! - [`registry`] keeps revisions and allocates IDs within one project scope.
//! - [`span`] defines byte ranges and their buffer identities.
//! - [`uri`] validates host document identities.

mod construction;
mod file;
mod registry;
mod span;
mod uri;

pub use construction::{JoinText, ReplaceText};
pub use file::{Position, SourceError, SourceFile};
pub use registry::{SourceRegistry, SourceRegistryError};
pub use span::{BytePos, FileId, Span, Spanned};
pub use uri::{SourceUri, SourceUriError};
