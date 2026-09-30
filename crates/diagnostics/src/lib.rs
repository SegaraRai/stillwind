//! Typed compilation errors, advisory lint findings, and source highlights.
//!
//! Construct a [`CompileError`] from its kind or a [`LintFinding`] from a
//! [`LintDiagnostic`] and a source span. Causes own message templates; [`LintKind`]
//! identifies a named lint rule, and [`LintSettings`] stores its configuration.
//! Wrappers retain ordered labels and notes. Compilation errors may also carry an
//! opaque display source name. The first label is the primary location and controls
//! lint suppression. Detection belongs to the frontend, IR validation, and compiler
//! passes, not here.
//!
//! `compilation` defines fatal causes; `lint` defines advisory causes and settings;
//! `label` defines shared highlights. `report` resolves source-bound public batches.
//! Source coordinates remain owned by [`stillwind_source`].

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

mod compilation;
mod label;
mod lint;
mod report;

pub use compilation::{
    CompileError, CompileErrorKind, SourceFailureContext, VariantInputErrorKind,
};
pub use label::{DiagnosticInfo, FixAction, Label, Labels, SourceEdit, SourceRange};
pub use lint::{
    LintConfig, LintDiagnostic, LintDirective, LintDirectiveKind, LintDirectiveRules, LintFinding,
    LintKind, LintSettings, LintSeverity, UtilityFailure, collect_lint_directives,
    filter_lint_findings,
};
pub use report::{
    AggregateError, DiagnosticBatch, DiagnosticCause, DiagnosticSeverity, LocatedDiagnostic,
    LocatedLabel, ReportError, Reporter, ResolvedAction, ResolvedDiagnostic, ResolvedEdit,
    ResolvedLabel, ResolvedLocation,
};
