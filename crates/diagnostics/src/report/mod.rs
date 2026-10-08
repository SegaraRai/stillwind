//! Source-bound diagnostic reporting for one analysis or update.
//!
//! Producers build labels from registered source revisions. [`Reporter::report`]
//! accepts those source-bound labels and [`Reporter::finish`] resolves one batch.
//!
//! `located` binds producer causes and labels to immutable sources. `resolved`
//! defines revision-pinned host diagnostics and verifies proposed edits. `reporter`
//! applies lint policy, validates labels and actions, and resolves a complete batch.

mod located;
mod reporter;
mod resolved;

pub use located::{DiagnosticCause, LocatedDiagnostic, LocatedLabel};
pub use reporter::{AggregateError, ReportError, Reporter};
pub use resolved::{
    DiagnosticBatch, DiagnosticSeverity, ResolvedAction, ResolvedDiagnostic, ResolvedEdit,
    ResolvedLabel, ResolvedLocation,
};
