//! Typed compilation errors, lint findings, and source highlights.
//!
//! Construct a [`CompileError`] from a [`CompileDiagnostic`] or a [`LintFinding`] from
//! a [`LintDiagnostic`] and a source span. Causes own message templates; [`LintKind`]
//! identifies a named lint rule, and [`LintSettings`] stores its configuration.
//! Wrappers retain ordered labels and notes. Compilation errors may also carry an
//! opaque display source name. The first label is the primary location and controls
//! lint suppression. Detection belongs to the frontend, IR validation, and compiler
//! passes, not here.
//!
//! ## Error and lint boundary
//!
//! Each macro call lowers one to one into a runtime form, and invalid input also has a
//! defined runtime meaning. A compilation error means that no artifact can be produced:
//!
//! - the compiler cannot read the source, such as a parse, source-map, or projection
//!   failure;
//! - the compiler cannot choose a runtime form, such as missing checker facts, a macro
//!   use outside a supported position, or a framework conflict;
//! - a constraint outside the one-to-one lowering fails, such as a composition cycle,
//!   CSS ordering, a generated-name collision, or a missing required slot;
//! - the environment or configuration is invalid.
//!
//! A lint finding never changes output. Input lints report contributions that lower as
//! empty and conditions that lower as false, such as unknown or invalid utilities,
//! variants, entries, and options. Policy lints report valid output that violates
//! project rules. Severity and suppression change only reporting.
//!
//! `compilation` defines fatal causes; `lint` defines lint causes and settings;
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
    CheckerFactsSubject, CompileDiagnostic, CompileError, CompileErrorClass, CompileErrorCode,
    SourceFailureContext,
};
pub use label::{DiagnosticInfo, FixAction, Label, LabelScope, Labels, SourceEdit, SourceRange};
pub use lint::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    LengthUnitRule, LengthUnitsConfig, LengthUnitsOverride, LintConfig, LintDiagnostic,
    LintDirective, LintDirectiveKind, LintDirectiveRules, LintFinding, LintKind, LintOverrides,
    LintPreset, LintSettings, LintSeverity, RestrictedDeclarationRule,
    RestrictedDeclarationsConfig, RestrictedDeclarationsOverride, RestrictedThemeTokensConfig,
    RestrictedThemeTokensOverride, RestrictedUtilitiesConfig, RestrictedUtilitiesOverride,
    RestrictedUtilityRule, RootFontSize, UtilityFailure, VariantInputErrorKind,
    collect_lint_directives, filter_lint_findings, finish_lint_findings,
};
pub use report::{
    AggregateError, DiagnosticBatch, DiagnosticCause, DiagnosticSeverity, LocatedDiagnostic,
    LocatedLabel, ReportError, Reporter, ResolvedAction, ResolvedDiagnostic, ResolvedEdit,
    ResolvedLabel, ResolvedLocation,
};
