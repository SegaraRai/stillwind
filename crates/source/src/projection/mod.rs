//! Validated correspondence from generated JavaScript back to the original source.
//!
//! A [`ProjectionMap`] is one of the two transport formats that a host converter
//! produces: a source map v3 document (points) or a Volar-style mapping table
//! (intervals). [`ProjectionIndex`] loads either format against the generated (shadow)
//! and original (editable) [`SourceFile`](crate::SourceFile) and answers queries about
//! the correspondence between them.
//!
//! Exact-span verification succeeds only when the mapped original text equals the
//! retained projected text; coordinate lookup alone does not verify text equality.
//! Converters rewrite code, so a correspondence that only looks plausible must fail
//! instead of producing an edit against the wrong characters. Neither loader
//! extrapolates one transport format into the other.
//!
//! Modules:
//!
//! - [`index`] owns the validated index and exact-source queries.
//! - [`v3`] validates point maps.
//! - [`volar`] validates interval tables.

mod index;
mod v3;
mod volar;

pub use index::{ProjectionError, ProjectionIndex, ProjectionMap, SourceProvenance};
pub use volar::VolarMapping;

#[cfg(test)]
mod tests;
