//! Expression-level type facts, keyed by source span.
//!
//! A [`TypeFact`] says what one expression in a module denotes after type checking, so
//! the frontend can treat an imported or conditionally selected ref value as a finite
//! candidate set without solving TypeScript itself. Facts are keyed by the byte span of
//! the expression in the module's original source; a fact whose span does not slice
//! that source is invalid, and the whole payload is discarded rather than partially
//! applied.

use serde::{Deserialize, Serialize};

use super::symbolic::ExportedValue;

/// What one checked expression denotes.
///
/// The fact is a claim by the host checker; consumers must check the enclosing
/// [`super::TypeFacts`] revisions before trusting it, and must fall back to local analysis
/// when it is missing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeFact {
    /// UTF-8 byte offset of the expression start in the original source.
    pub start: u32,
    /// UTF-8 byte offset of the expression end, exclusive.
    pub end: u32,
    /// Symbolic value of the expression.
    pub value: ExportedValue,
}
