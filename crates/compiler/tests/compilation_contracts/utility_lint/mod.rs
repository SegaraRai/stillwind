//! Source utility policies exercised through the public compiler API.
//!
//! [`variants`] checks breakpoint and direct styling policies under variants.
//! [`candidates`] checks redundant and contradicting variants, clamped and invalid
//! values, and untyped variables, with their ranges and fixes. [`restrictions`] checks
//! source value, utility, declaration, and token restrictions. [`overlap`] checks
//! competing contributions, duplicates, and suppression usage. [`units`] checks length
//! units and logical css property policies. [`support`] supplies the builders shared by
//! these cases. [`value_properties`] checks custom properties that supply values for
//! ordinary properties.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{
    ArbitrarySpacingOverride, CanonicalUtilitiesOverride, ConflictingUtilitiesOverride,
    ImportantOverride, LintFinding, LintKind, LintOverrides, LintSeverity,
    LogicalPropertiesOverride, RestrictedDeclarationRule, RestrictedDeclarationsOverride,
    RestrictedLengthUnitRule, RestrictedLengthUnitsOverride, RestrictedThemeTokensOverride,
    RestrictedUtilitiesOverride, RestrictedUtilityRule,
};
use stillwind_test_support::with_defaults;

use super::compile;
use super::support::{CompilationFixture, compile_with_options};

mod candidates;
mod overlap;
mod restrictions;
mod support;
mod units;
mod value_properties;
mod variants;

use support::{CSS, VALUE_CSS, kinds, kinds_in, policy_options};
