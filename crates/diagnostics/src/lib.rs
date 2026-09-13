//! Typed compilation errors, advisory lint findings, and source highlights.
//!
//! Construct a [`CompileError`] or [`LintFinding`] from its kind and a source span.
//! Kinds own message templates and stable codes; wrappers retain ordered labels and
//! notes; compilation errors may also carry an opaque display source name. The first
//! label is the primary location and controls lint suppression for lint findings.
//! Detection belongs to the frontend, IR validation, and compiler passes, not here.
//!
//! [`compilation`] defines fatal causes; [`lint`] defines advisory causes; [`label`]
//! defines shared highlights. [`suppression`] parses and applies `swlint-*` directives
//! only to lint findings. Source coordinates remain owned by [`stillwind_source`].

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
mod suppression;

pub use compilation::{CompileError, CompileErrorKind, SourceFailureContext};
pub use label::Label;
pub use lint::{LintFinding, LintKind, UtilityFailure};
pub use suppression::{
    LintDirective, LintDirectiveKind, collect_lint_directives, filter_lint_findings,
};
