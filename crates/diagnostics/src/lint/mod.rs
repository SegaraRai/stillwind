//! Lint rules, diagnostic contents, and source-located findings.
//!
//! Construct [`LintFinding`] from a [`LintDiagnostic`] and a source span. Producers
//! detect findings, and reporters apply the configured severity and suppression.
//!
//! `config` defines named rules, setting variants, and severity lookup. `diagnostic`
//! defines detected causes, message templates, and utility syntax classifications.
//! `finding` attaches source labels, notes, and actions to a detected cause.
//! `suppression` parses `swlint-*` comments and filters findings by their primary
//! spans.

mod config;
mod diagnostic;
mod finding;
mod suppression;

pub use config::{
    ArbitrarySpacingConfig, ArbitrarySpacingOverride, CanonicalUtilitiesConfig,
    CanonicalUtilitiesOverride, ConflictingUtilitiesConfig, ConflictingUtilitiesOverride,
    LintCategory, LintConfig, LintKind, LintOverrides, LintPreset, LintSettings, LintSeverity,
    RestrictedDeclarationRule, RestrictedDeclarationsConfig, RestrictedDeclarationsOverride,
    RestrictedLengthUnitRule, RestrictedLengthUnitsConfig, RestrictedLengthUnitsOverride,
    RestrictedThemeTokensConfig, RestrictedThemeTokensOverride, RestrictedUtilitiesConfig,
    RestrictedUtilitiesOverride, RestrictedUtilityRule,
};
pub use diagnostic::{LintDiagnostic, UtilityFailure, VariantInputErrorKind};
pub use finding::LintFinding;
pub use suppression::{
    LintDirective, LintDirectiveKind, LintDirectiveRules, collect_lint_directives,
    filter_lint_findings, finish_lint_findings,
};

pub(crate) use suppression::suppressing_directive;
