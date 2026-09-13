//! Validated correspondence from generated JavaScript back to the original source.
//!
//! A [`ProjectionMap`] is one of the two transport formats a host converter produces: a
//! source map v3 document (points) or a Volar-style mapping table (intervals).
//! [`ProjectionIndex`] loads either format against the generated (shadow) and original
//! (editable) [`crate::SourceFile`]s and answers queries about the correspondence between
//! them.
//!
//! Exact-span verification succeeds only when the mapped original text equals the
//! retained projected text. Coordinate lookup alone does not verify text equality.
//! Converters rewrite code, so a correspondence that merely looks plausible must fail
//! rather than produce an edit against the wrong characters.
//!
//! `index` owns the validated index and exact-source queries; `v3` validates point
//! maps; `volar` validates interval tables. Neither loader extrapolates one transport
//! into the other.

mod index;
mod v3;
mod volar;

pub use index::{ProjectionError, ProjectionIndex, ProjectionMap, SourceProvenance};
pub use volar::VolarMapping;
