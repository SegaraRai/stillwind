//! Detection policies and partial overrides for lint rules with options.

use serde::{Deserialize, Serialize};

use super::LintSeverity;

/// Positive finite number of CSS pixels per rem, used only when spacing uses rem.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RootFontSize(f64);

impl RootFontSize {
    pub(super) const DEFAULT: Self = Self(16.0);

    /// Returns `None` for zero, negative, or nonfinite values.
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    /// Returns the validated number of pixels per rem.
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Eq for RootFontSize {}

impl<'de> Deserialize<'de> for RootFontSize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?)
            .ok_or_else(|| serde::de::Error::custom("rootFontSize must be positive and finite"))
    }
}

/// Effective canonical lint policy after compiler defaults and project settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalUtilitiesConfig {
    /// Off skips detection and reporting; deny reports an error with the same fixes.
    pub severity: LintSeverity,
    /// Enables conversion of pixel spacing when the active spacing scale uses rem.
    pub px_to_rem: bool,
    /// Pixels per rem used by spacing conversion. Other utility domains retain their units.
    pub root_font_size: RootFontSize,
}

impl Default for CanonicalUtilitiesConfig {
    fn default() -> Self {
        Self {
            severity: LintSeverity::Warn,
            px_to_rem: true,
            root_font_size: RootFontSize::DEFAULT,
        }
    }
}

/// Project fields that override only the explicitly supplied parts of canonical lint policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CanonicalUtilitiesOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission keeps the default conversion policy.
    pub px_to_rem: Option<bool>,
    /// Omission keeps the default 16 pixels per rem.
    pub root_font_size: Option<RootFontSize>,
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
pub struct LengthUnitRule {
    /// Case-insensitive property globs supporting `*` and `?`.
    pub properties: Vec<String>,
    /// Length unit names compared without ASCII case; zero lengths are exempt.
    pub disallow: Vec<String>,
}

/// Property-specific unit restrictions in precedence order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LengthUnitsConfig {
    /// Reporting level of forbidden units.
    pub severity: LintSeverity,
    /// The last matching rule replaces earlier restrictions.
    pub rules: Vec<LengthUnitRule>,
}

/// Explicit fields overriding the length unit policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LengthUnitsOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission preserves existing rules; a supplied list replaces them.
    pub rules: Option<Vec<LengthUnitRule>>,
}

/// Theme custom properties forbidden at direct use sites.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestrictedThemeTokensConfig {
    /// Reporting level of forbidden token references.
    pub severity: LintSeverity,
    /// Case-sensitive complete custom-property name globs supporting `*` and `?`.
    pub tokens: Vec<String>,
}

/// Explicit fields overriding the restricted theme token policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RestrictedThemeTokensOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// Omission preserves existing patterns; a supplied list replaces them.
    pub tokens: Option<Vec<String>>,
}

/// Arbitrary spacing restrictions selected by CSS property domain.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArbitrarySpacingConfig {
    /// Reporting level for explicit nonzero lengths and percentages.
    pub severity: LintSeverity,
    /// Additional case-insensitive property globs. Margin, padding, and gap are always checked.
    pub properties: Vec<String>,
}

/// Explicit fields overriding the arbitrary spacing policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArbitrarySpacingOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// A supplied list replaces additional property globs.
    pub properties: Option<Vec<String>>,
}

/// A forbidden utility spelling and the explanation presented to its author.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestrictedUtilityRule {
    /// Case-sensitive full utility glob, excluding enclosing variants and important modifiers.
    pub pattern: String,
    /// Optional project-specific explanation.
    pub message: Option<String>,
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
    pub properties: Vec<String>,
    /// Case-insensitive complete serialized value globs; strings and URLs remain CSS text.
    pub values: Vec<String>,
    /// Optional project-specific explanation.
    pub message: Option<String>,
}

/// Forbidden combinations of resolved properties and values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestrictedDeclarationsConfig {
    /// Reporting level of forbidden declarations.
    pub severity: LintSeverity,
    /// Every matching restriction applies; later rules do not override earlier rules.
    pub rules: Vec<RestrictedDeclarationRule>,
}

/// Explicit fields overriding forbidden declarations.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RestrictedDeclarationsOverride {
    /// Omission uses the rule default.
    pub severity: Option<LintSeverity>,
    /// A supplied list replaces declaration restrictions.
    pub rules: Option<Vec<RestrictedDeclarationRule>>,
}
