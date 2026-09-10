//! Source buffers, spans, line indexes, and diagnostics used by every compiler pass.

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
