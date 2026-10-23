//! Detection policies and partial overrides for lint rules with options.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use super::LintSeverity;

/// Effective canonical lint policy after compiler defaults and project settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalUtilitiesConfig {
    /// Off skips detection and reporting; deny reports an error with the same fixes.
    pub severity: LintSeverity,
    /// Enables conversion of pixel spacing when the active spacing scale uses rem.
    pub px_to_rem: bool,
}

impl Default for CanonicalUtilitiesConfig {
    fn default() -> Self {
        Self {
            severity: LintSeverity::Warn,
            px_to_rem: true,
        }
    }
}

/// Project fields that override only the explicitly supplied parts of canonical lint
/// policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CanonicalUtilitiesOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission keeps the default conversion policy.
    pub px_to_rem: Option<bool>,
}

/// Effective policy for `invalid-marker-use` after compiler defaults and project
/// settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerUseConfig {
    /// Off skips detection and reporting.
    pub severity: LintSeverity,
    /// Marker class names whose findings are not reported, compared exactly with the
    /// decoded class name, such as `group` or `foo/bar`.
    pub ignored_classes: Vec<LeanStr>,
}

impl MarkerUseConfig {
    /// Returns whether findings about the marker class `class` are reported.
    pub fn reports_class(&self, class: &str) -> bool {
        !self.ignored_classes.iter().any(|ignored| ignored == class)
    }
}

/// Project fields that override only the explicitly supplied parts of the
/// `invalid-marker-use` policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MarkerUseOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission preserves existing names; a supplied list replaces them.
    pub ignored_classes: Option<Vec<LeanStr>>,
}

/// Effective policy for `prefer-variant-directive` after compiler defaults and project
/// settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantDirectiveConfig {
    /// Off skips detection and reporting; deny reports an error with the same fixes.
    pub severity: LintSeverity,
    /// Lets a `px` length in a `@media` condition match a `rem` length of a variant at
    /// 16px = 1rem.
    pub px_to_rem: bool,
}

impl Default for VariantDirectiveConfig {
    fn default() -> Self {
        Self {
            severity: LintSeverity::Warn,
            px_to_rem: false,
        }
    }
}

/// Project fields that override only the explicitly supplied parts of the
/// `prefer-variant-directive` policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VariantDirectiveOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission keeps the default, which compares lengths without unit conversion.
    pub px_to_rem: Option<bool>,
}

/// Effective policy for declarations that compete within a style call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictingUtilitiesConfig {
    /// Reporting level of utility overlaps.
    pub severity: LintSeverity,
    /// Allows a narrower shorthand or longhand to override part of a wider property.
    pub allow_shorthand_overrides: bool,
}

impl Default for ConflictingUtilitiesConfig {
    fn default() -> Self {
        Self {
            severity: LintSeverity::Warn,
            allow_shorthand_overrides: false,
        }
    }
}

/// Explicit fields overriding the utility overlap policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConflictingUtilitiesOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission preserves the shorthand policy.
    pub allow_shorthand_overrides: Option<bool>,
}

/// One unit restriction selected by CSS property name.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestrictedLengthUnitRule {
    /// Case-insensitive property globs supporting `*` and `?`.
    pub properties: Vec<LeanStr>,
    /// Length unit names compared without ASCII case; zero lengths are exempt.
    pub disallow: Vec<LeanStr>,
}

/// Property-specific unit restrictions in precedence order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestrictedLengthUnitsConfig {
    /// Reporting level of forbidden units.
    pub severity: LintSeverity,
    /// Include fixed declarations from `(implementation)` definitions. Defaults to
    /// false.
    pub check_implementation: bool,
    /// The last matching rule replaces earlier restrictions.
    pub rules: Vec<RestrictedLengthUnitRule>,
}

/// Explicit fields overriding the length unit policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RestrictedLengthUnitsOverride {
    /// Omission preserves the existing severity, or uses warn without a prior policy.
    pub severity: Option<LintSeverity>,
    /// Omission preserves the existing choice, or uses false without a prior policy.
    pub check_implementation: Option<bool>,
    /// Omission preserves existing rules; a supplied list replaces them.
    pub rules: Option<Vec<RestrictedLengthUnitRule>>,
}

/// Logical-property checks for resolved declarations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogicalPropertiesConfig {
    /// Reporting level for physical properties and directional values.
    pub severity: LintSeverity,
    /// Include fixed declarations from `(implementation)` definitions. Defaults to
    /// false.
    pub check_implementation: bool,
}

/// Explicit fields overriding logical-property checks.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogicalPropertiesOverride {
    /// Omission preserves the reporting level.
    pub severity: Option<LintSeverity>,
    /// Omission preserves the existing choice, or uses false without a prior policy.
    pub check_implementation: Option<bool>,
}

/// Effective restrictions on theme token names referenced by utilities and variants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestrictedThemeTokensConfig {
    /// Reporting level of forbidden token references.
    pub severity: LintSeverity,
    /// Include token references from fixed `(implementation)` declarations. Defaults
    /// to true. Tokens selected through placeholders or variant arguments stay checked
    /// when this is false.
    pub check_implementation: bool,
    /// Case-sensitive complete custom-property name globs supporting `*` and `?`.
    pub tokens: Vec<LeanStr>,
}

/// Partial updates to the restricted theme token policy for `LintOverrides::resolve`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RestrictedThemeTokensOverride {
    /// Omission preserves the existing severity, or uses warn without a prior policy.
    pub severity: Option<LintSeverity>,
    /// Omission preserves the existing choice, or uses true without a prior policy.
    pub check_implementation: Option<bool>,
    /// Omission preserves existing patterns; a supplied list replaces them.
    pub tokens: Option<Vec<LeanStr>>,
}

/// Arbitrary spacing restrictions selected by CSS property domain.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArbitrarySpacingConfig {
    /// Reporting level for explicit nonzero lengths and percentages.
    pub severity: LintSeverity,
    /// Additional case-insensitive property globs. Margin, padding, and gap are always
    /// checked.
    pub properties: Vec<LeanStr>,
}

/// Explicit fields overriding the arbitrary spacing policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArbitrarySpacingOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// A supplied list replaces additional property globs.
    pub properties: Option<Vec<LeanStr>>,
}

/// A forbidden utility spelling and the explanation presented to its author.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestrictedUtilityRule {
    /// Case-sensitive full utility glob, excluding enclosing variants and important
    /// modifiers.
    pub pattern: LeanStr,
    /// Optional project-specific explanation.
    pub message: Option<LeanStr>,
}

/// Utility spellings forbidden independently of their resolved declarations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestrictedUtilitiesConfig {
    /// Reporting level of forbidden utilities.
    pub severity: LintSeverity,
    /// Each matching rule reports its optional explanation.
    pub utilities: Vec<RestrictedUtilityRule>,
}

/// Explicit fields overriding forbidden utility spellings.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RestrictedUtilitiesOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// A supplied list replaces forbidden utility rules.
    pub utilities: Option<Vec<RestrictedUtilityRule>>,
}

/// A restriction on resolved CSS declarations and their parsed longhands.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestrictedDeclarationRule {
    /// Case-insensitive complete CSS property globs.
    pub properties: Vec<LeanStr>,
    /// Case-insensitive complete serialized value globs; strings and URLs remain CSS
    /// text.
    pub values: Vec<LeanStr>,
    /// Optional project-specific explanation.
    pub message: Option<LeanStr>,
}

/// Effective restrictions on resolved property and value combinations, including
/// parsed shorthand components.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestrictedDeclarationsConfig {
    /// Reporting level of forbidden declarations.
    pub severity: LintSeverity,
    /// Include fixed `(implementation)` declarations. Defaults to true. Declarations
    /// with executable placeholders and unmarked extensions stay checked when false.
    pub check_implementation: bool,
    /// Every matching restriction applies; later rules do not override earlier rules.
    pub rules: Vec<RestrictedDeclarationRule>,
}

/// Partial updates to declaration restrictions for `LintOverrides::resolve`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RestrictedDeclarationsOverride {
    /// Omission preserves the existing severity, or uses warn without a prior policy.
    pub severity: Option<LintSeverity>,
    /// Omission preserves the existing choice, or uses true without a prior policy.
    pub check_implementation: Option<bool>,
    /// A supplied list replaces declaration restrictions.
    pub rules: Option<Vec<RestrictedDeclarationRule>>,
}

/// Effective restrictions on important utility declarations and source modifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportantConfig {
    /// Reporting level for important declarations and source modifiers.
    pub severity: LintSeverity,
    /// Include fixed `(implementation)` declarations. Defaults to true. Source `!`
    /// modifiers and declarations with executable placeholders stay checked when false.
    pub check_implementation: bool,
}

/// Partial updates to importance restrictions for `LintOverrides::resolve`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportantOverride {
    /// Omission preserves the existing severity, or uses warn without a prior policy.
    pub severity: Option<LintSeverity>,
    /// Omission preserves the existing choice, or uses true without a prior policy.
    pub check_implementation: Option<bool>,
}
