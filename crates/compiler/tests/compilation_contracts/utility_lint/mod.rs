//! Source utility policies exercised through the public compiler API.
//!
//! `variants` checks breakpoint and direct styling policies under variants.
//! `restrictions` checks source value, utility, declaration, and token restrictions.
//! `overlap` checks competing contributions, duplicates, and suppression usage.
//! `units` checks length units and logical css property policies.
//! `support` supplies the builders shared by these cases.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{
    ArbitrarySpacingOverride, CanonicalUtilitiesOverride, ConflictingUtilitiesOverride,
    LengthUnitRule, LengthUnitsOverride, LintFinding, LintKind, LintOverrides, LintSeverity,
    RestrictedDeclarationRule, RestrictedDeclarationsOverride, RestrictedThemeTokensOverride,
    RestrictedUtilitiesOverride, RestrictedUtilityRule,
};
use stillwind_test_support::with_defaults;

use super::compile;
use super::support::{CompilationFixture, compile_with_options};

mod overlap;
mod restrictions;
mod support;
mod units;
mod variants;

use support::{CSS, VALUE_CSS, kinds, kinds_in, policy_options};
