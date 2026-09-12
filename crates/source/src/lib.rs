//! Source buffers, spans, line indexes, diagnostics, and lint suppression.
//!
//! Two coordinate systems are kept apart. Rust passes address text in UTF-8 bytes
//! ([`BytePos`], [`Span`]); hosts and source maps address it in UTF-16 code units
//! ([`Position`], [`Utf16Offset`]). [`CoordinateIndex`] is the only sanctioned
//! translation between them, and it rejects offsets inside a surrogate pair.
//!
//! [`SourceFile`] owns immutable text and validates every slice, so a [`Span`] is
//! meaningless outside the file that produced it. [`Fingerprint`] provides
//! process-independent content identity, and the lint module owns the `swlint-*`
//! suppression rules shared by all passes. The crate performs no I/O.

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
mod diagnostics;
mod fingerprint;
mod lint;
mod projection;
mod text;

pub use coordinates::{CoordinateError, CoordinateIndex, Utf16Offset};
pub use diagnostics::{CompileError, Label, LintFinding};
pub use fingerprint::Fingerprint;
pub use lint::{LintDirective, LintDirectiveKind, collect_lint_directives, filter_lint_findings};
pub use projection::{
    ProjectionError, ProjectionIndex, ProjectionMap, SourceProvenance, VolarMapping,
};
pub use text::{
    BytePos, FileId, Position, SourceError, SourceFile, Span, Spanned, normalized_module_path,
    range_of,
};
