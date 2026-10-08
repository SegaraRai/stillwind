//! Lint rule identities and reporting settings shared by producers and hosts.
//!
//! Parse rule names with [`LintKind::from_name`] and insert [`LintConfig`] values into
//! [`LintSettings`]. Configured detection and reporting of unconfigured findings use
//! separate lookups. Serialization emits the map of names to severities used by hosts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer};

/// Configured reporting level of a lint finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    /// Report a warning.
    Warn,
    /// Report an error without discarding suggested fixes.
    Deny,
    /// Omit the finding and its suggested fixes.
    Off,
}

impl LintSeverity {
    /// Returns the CSS configuration and host-protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Off => "off",
        }
    }

    /// Parses an exact CSS lint level; unsupported values return `None`.
    pub fn from_css(value: &str) -> Option<Self> {
        match value {
            "warn" => Some(Self::Warn),
            "deny" => Some(Self::Deny),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

macro_rules! define_lint_rules {
    ($( $variant:ident => ($name:literal, $default:expr), )+) => {
        /// A lint rule identified by one stable CSS and suppression name.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum LintKind {
            $(
                #[doc = concat!("The `", $name, "` rule.")]
                #[serde(rename = $name)]
                $variant,
            )+
        }

        impl LintKind {
            /// Every supported lint rule, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Returns the stable CSS, suppression, and host-protocol name.
            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)+ }
            }

            /// Parses an exact rule name; unknown names return `None`.
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($name => Some(Self::$variant),)+ _ => None }
            }

            /// Returns the default setting when detection is configured by default.
            /// Rules without a setting can still be reported when a producer detects them.
            pub const fn default_config(self) -> Option<LintConfig> {
                let severity: Option<LintSeverity> = match self {
                    $(Self::$variant => $default,)+
                };
                match severity {
                    Some(level) => Some(LintConfig::new(self, level)),
                    None => None,
                }
            }
        }

        /// Configuration for one lint rule, including its reporting severity.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum LintConfig {
            $(
                #[doc = concat!("Configuration for `", $name, "`.")]
                $variant(#[doc = "Settings for detection and reporting of this rule."] lint_config_type!($variant)),
            )+
        }

        impl LintConfig {
            /// Constructs the setting variant corresponding to `kind` with `severity`.
            pub const fn new(kind: LintKind, severity: LintSeverity) -> Self {
                match kind { $(LintKind::$variant => Self::$variant(lint_config_value!($variant, severity)),)+ }
            }

            /// Returns the rule this setting configures.
            pub const fn kind(&self) -> LintKind {
                match self { $(Self::$variant(_) => LintKind::$variant,)+ }
            }

            /// Returns the configured reporting severity.
            pub const fn severity(&self) -> LintSeverity {
                match self { $(Self::$variant(config) => lint_config_severity!($variant, config),)+ }
            }
        }
    };
}

macro_rules! lint_config_type {
    (NoArbitrarySpacing) => {
        ArbitrarySpacingConfig
    };
    (NoRestrictedUtilities) => {
        RestrictedUtilitiesConfig
    };
    (NoRestrictedDeclarations) => {
        RestrictedDeclarationsConfig
    };
    (ConflictingUtilities) => {
        ConflictingUtilitiesConfig
    };
    (LengthUnits) => {
        LengthUnitsConfig
    };
    (NoRestrictedThemeTokens) => {
        RestrictedThemeTokensConfig
    };
    (CanonicalUtilities) => {
        CanonicalUtilitiesConfig
    };
    ($variant:ident) => {
        LintSeverity
    };
}

macro_rules! lint_config_value {
    (NoArbitrarySpacing, $severity:expr) => {
        ArbitrarySpacingConfig {
            severity: $severity,
            properties: Vec::new(),
        }
    };
    (NoRestrictedUtilities, $severity:expr) => {
        RestrictedUtilitiesConfig {
            severity: $severity,
            utilities: Vec::new(),
        }
    };
    (NoRestrictedDeclarations, $severity:expr) => {
        RestrictedDeclarationsConfig {
            severity: $severity,
            rules: Vec::new(),
        }
    };
    (ConflictingUtilities, $severity:expr) => {
        ConflictingUtilitiesConfig {
            severity: $severity,
            allow_shorthand_overrides: false,
        }
    };
    (LengthUnits, $severity:expr) => {
        LengthUnitsConfig {
            severity: $severity,
            rules: Vec::new(),
        }
    };
    (NoRestrictedThemeTokens, $severity:expr) => {
        RestrictedThemeTokensConfig {
            severity: $severity,
            tokens: Vec::new(),
        }
    };
    (CanonicalUtilities, $severity:expr) => {
        CanonicalUtilitiesConfig {
            severity: $severity,
            px_to_rem: true,
            root_font_size: RootFontSize(16.0),
        }
    };
    ($variant:ident, $severity:expr) => {
        $severity
    };
}

macro_rules! lint_config_severity {
    (NoArbitrarySpacing, $config:expr) => {
        $config.severity
    };
    (NoRestrictedUtilities, $config:expr) => {
        $config.severity
    };
    (NoRestrictedDeclarations, $config:expr) => {
        $config.severity
    };
    (ConflictingUtilities, $config:expr) => {
        $config.severity
    };
    (LengthUnits, $config:expr) => {
        $config.severity
    };
    (NoRestrictedThemeTokens, $config:expr) => {
        $config.severity
    };
    (CanonicalUtilities, $config:expr) => {
        $config.severity
    };
    ($variant:ident, $config:expr) => {
        *$config
    };
}

/// Positive finite number of CSS pixels per rem, used only when spacing uses rem.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RootFontSize(f64);

impl RootFontSize {
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

/// Effective canonical lint policy after stylesheet severity and project overrides.
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
            root_font_size: RootFontSize(16.0),
        }
    }
}

/// Project fields that override only the explicitly supplied parts of canonical lint policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CanonicalUtilitiesOverride {
    /// Omission keeps the stylesheet severity.
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
    /// Omission preserves the stylesheet reporting level.
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
    /// Omission preserves the stylesheet reporting level.
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
    /// Omission preserves the stylesheet reporting level.
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
    /// Omission preserves the stylesheet reporting level.
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
    /// Omission preserves the stylesheet reporting level.
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
    /// Omission preserves the stylesheet reporting level.
    pub severity: Option<LintSeverity>,
    /// A supplied list replaces declaration restrictions.
    pub rules: Option<Vec<RestrictedDeclarationRule>>,
}

/// Project overrides applied after defaults and CSS severity directives.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct LintOverrides {
    /// Reporting level for explicit color literals.
    pub no_arbitrary_colors: Option<LintSeverity>,
    /// Restrictions on explicit spacing lengths and percentages.
    pub no_arbitrary_spacing: Option<ArbitrarySpacingOverride>,
    /// Reporting level for external class contributions in style objects.
    pub no_class_property: Option<LintSeverity>,
    /// Forbidden utility spellings.
    pub no_restricted_utilities: Option<RestrictedUtilitiesOverride>,
    /// Forbidden resolved property and value combinations.
    pub no_restricted_declarations: Option<RestrictedDeclarationsOverride>,
    /// Reporting level for important declarations.
    pub no_important: Option<LintSeverity>,
    /// Reporting level for unknown suppression names and malformed directives.
    pub valid_lint_suppressions: Option<LintSeverity>,
    /// Reporting level for disable directives that suppress no enabled findings.
    pub no_unused_lint_suppressions: Option<LintSeverity>,
    /// Whether variants must retain the element receiving the style.
    pub prefer_direct_styling: Option<LintSeverity>,
    /// Reporting level for repeated utility candidates.
    pub duplicate_utilities: Option<LintSeverity>,
    /// Overlap policy for utilities supplied directly by one call.
    pub conflicting_utilities: Option<ConflictingUtilitiesOverride>,
    /// Reporting level for direction-dependent physical declarations.
    pub prefer_logical_properties: Option<LintSeverity>,
    /// Property-specific unit restrictions.
    pub length_units: Option<LengthUnitsOverride>,
    /// Theme token names forbidden at their direct use sites.
    pub no_restricted_theme_tokens: Option<RestrictedThemeTokensOverride>,
    /// Reporting level for viewport size conditions supplied as arbitrary values.
    pub no_arbitrary_breakpoints: Option<LintSeverity>,
    /// Field-wise canonical policy overrides.
    pub canonical_utilities: Option<CanonicalUtilitiesOverride>,
    /// Optional unsorted-source diagnostic. It is disabled by default.
    pub sort_utilities: Option<LintSeverity>,
}

impl LintOverrides {
    /// Resolves project fields against the stylesheet settings without changing either input.
    pub fn resolve(&self, stylesheet: &LintSettings) -> LintSettings {
        let mut settings = stylesheet.clone();
        for (kind, severity) in [
            (LintKind::NoArbitraryColors, self.no_arbitrary_colors),
            (LintKind::NoClassProperty, self.no_class_property),
            (LintKind::NoImportant, self.no_important),
            (
                LintKind::ValidLintSuppressions,
                self.valid_lint_suppressions,
            ),
            (
                LintKind::NoUnusedLintSuppressions,
                self.no_unused_lint_suppressions,
            ),
            (LintKind::PreferDirectStyling, self.prefer_direct_styling),
            (LintKind::DuplicateUtilities, self.duplicate_utilities),
            (
                LintKind::NoArbitraryBreakpoints,
                self.no_arbitrary_breakpoints,
            ),
            (
                LintKind::PreferLogicalProperties,
                self.prefer_logical_properties,
            ),
        ] {
            if let Some(severity) = severity {
                settings.insert(LintConfig::new(kind, severity));
            }
        }
        self.resolve_source_restrictions(&mut settings);
        if let Some(overrides) = &self.conflicting_utilities {
            let mut config = match settings.get(LintKind::ConflictingUtilities) {
                Some(LintConfig::ConflictingUtilities(config)) => config.clone(),
                _ => ConflictingUtilitiesConfig::default(),
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.allow_shorthand_overrides {
                config.allow_shorthand_overrides = value;
            }
            settings.insert(LintConfig::ConflictingUtilities(config));
        }
        if let Some(overrides) = &self.length_units {
            let mut config = match settings.get(LintKind::LengthUnits) {
                Some(LintConfig::LengthUnits(config)) => config.clone(),
                _ => LengthUnitsConfig {
                    severity: LintSeverity::Warn,
                    rules: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.rules {
                config.rules.clone_from(value);
            }
            settings.insert(LintConfig::LengthUnits(config));
        }
        if let Some(overrides) = &self.no_restricted_theme_tokens {
            let mut config = match settings.get(LintKind::NoRestrictedThemeTokens) {
                Some(LintConfig::NoRestrictedThemeTokens(config)) => config.clone(),
                _ => RestrictedThemeTokensConfig {
                    severity: LintSeverity::Warn,
                    tokens: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.tokens {
                config.tokens.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedThemeTokens(config));
        }
        if let Some(overrides) = &self.canonical_utilities {
            let mut config = match settings.get(LintKind::CanonicalUtilities) {
                Some(LintConfig::CanonicalUtilities(config)) => config.clone(),
                _ => CanonicalUtilitiesConfig::default(),
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = overrides.px_to_rem {
                config.px_to_rem = value;
            }
            if let Some(value) = overrides.root_font_size {
                config.root_font_size = value;
            }
            settings.insert(LintConfig::CanonicalUtilities(config));
        }
        if let Some(severity) = self.sort_utilities {
            settings.insert(LintConfig::SortUtilities(severity));
        }
        settings
    }

    fn resolve_source_restrictions(&self, settings: &mut LintSettings) {
        if let Some(overrides) = &self.no_arbitrary_spacing {
            let mut config = match settings.get(LintKind::NoArbitrarySpacing) {
                Some(LintConfig::NoArbitrarySpacing(config)) => config.clone(),
                _ => ArbitrarySpacingConfig {
                    severity: LintSeverity::Warn,
                    properties: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.properties {
                config.properties.clone_from(value);
            }
            settings.insert(LintConfig::NoArbitrarySpacing(config));
        }
        if let Some(overrides) = &self.no_restricted_utilities {
            let mut config = match settings.get(LintKind::NoRestrictedUtilities) {
                Some(LintConfig::NoRestrictedUtilities(config)) => config.clone(),
                _ => RestrictedUtilitiesConfig {
                    severity: LintSeverity::Warn,
                    utilities: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.utilities {
                config.utilities.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedUtilities(config));
        }
        if let Some(overrides) = &self.no_restricted_declarations {
            let mut config = match settings.get(LintKind::NoRestrictedDeclarations) {
                Some(LintConfig::NoRestrictedDeclarations(config)) => config.clone(),
                _ => RestrictedDeclarationsConfig {
                    severity: LintSeverity::Warn,
                    rules: Vec::new(),
                },
            };
            if let Some(value) = overrides.severity {
                config.severity = value;
            }
            if let Some(value) = &overrides.rules {
                config.rules.clone_from(value);
            }
            settings.insert(LintConfig::NoRestrictedDeclarations(config));
        }
    }
}

define_lint_rules! {
    NoArbitraryColors => ("no-arbitrary-colors", None),
    NoArbitrarySpacing => ("no-arbitrary-spacing", None),
    NoClassProperty => ("no-class-property", None),
    NoRestrictedUtilities => ("no-restricted-utilities", None),
    NoRestrictedDeclarations => ("no-restricted-declarations", None),
    NoImportant => ("no-important", None),
    ValidLintSuppressions => ("valid-lint-suppressions", Some(LintSeverity::Warn)),
    NoUnusedLintSuppressions => ("no-unused-lint-suppressions", None),
    PreferDirectStyling => ("prefer-direct-styling", None),
    DuplicateUtilities => ("duplicate-utilities", Some(LintSeverity::Warn)),
    ConflictingUtilities => ("conflicting-utilities", Some(LintSeverity::Warn)),
    PreferLogicalProperties => ("prefer-logical-properties", None),
    LengthUnits => ("length-units", None),
    NoRestrictedThemeTokens => ("no-restricted-theme-tokens", None),
    NoArbitraryBreakpoints => ("no-arbitrary-breakpoints", None),
    FrameworkImportMismatch => ("framework-import-mismatch", Some(LintSeverity::Warn)),
    MacroArgumentMutation => ("macro-argument-mutation", Some(LintSeverity::Deny)),
    MacroExecutionBoundary => ("macro-execution-boundary", Some(LintSeverity::Deny)),
    ValidAnchorScope => ("valid-anchor-scope", None),
    RequireRefAttachment => ("require-ref-attachment", None),
    ConflictingProviderProperties => ("conflicting-provider-properties", None),
    PreferTypedRelationships => ("prefer-typed-relationships", None),
    RequireElementScope => ("require-element-scope", Some(LintSeverity::Deny)),
    RequireViewTransitionName => ("require-view-transition-name", Some(LintSeverity::Warn)),
    SingleViewTransitionName => ("single-view-transition-name", Some(LintSeverity::Warn)),
    SingleViewTransitionClass => ("single-view-transition-class", Some(LintSeverity::Warn)),
    AmbiguousBaseOrder => ("ambiguous-base-order", Some(LintSeverity::Warn)),
    ImportedBaseInterference => ("imported-base-interference", Some(LintSeverity::Warn)),
    InvalidUtility => ("invalid-utility", None),
    UnknownUtility => ("unknown-utility", None),
    CanonicalUtilities => ("canonical-utilities", Some(LintSeverity::Warn)),
    SortUtilities => ("sort-utilities", None),
}

/// Settings indexed by rule, with at most one setting for each rule.
/// Absent settings return `false` from `enabled_for_detection`. Producers can detect
/// findings independently of that lookup; reporting an unconfigured rule uses `Warn`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintSettings {
    entries: BTreeMap<LintKind, LintConfig>,
}

impl LintSettings {
    /// Returns the settings configured by default for a stylesheet.
    pub fn with_defaults() -> Self {
        LintKind::ALL
            .iter()
            .filter_map(|kind| kind.default_config())
            .collect()
    }

    /// Adds or replaces the setting for `config.kind()`, returning the previous setting.
    pub fn insert(&mut self, config: LintConfig) -> Option<LintConfig> {
        self.entries.insert(config.kind(), config)
    }

    /// Returns the explicit or stylesheet-default setting for `kind`, if present.
    pub fn get(&self, kind: LintKind) -> Option<&LintConfig> {
        self.entries.get(&kind)
    }

    /// Returns whether a present setting enables detection; absent settings return `false`.
    pub fn enabled_for_detection(&self, kind: LintKind) -> bool {
        self.get(kind)
            .is_some_and(|config| config.severity() != LintSeverity::Off)
    }

    /// Returns the configured severity, or `Warn` for an unconfigured detected rule.
    pub fn reporting_severity(&self, kind: LintKind) -> LintSeverity {
        self.get(kind)
            .map_or(LintSeverity::Warn, LintConfig::severity)
    }

    /// Iterates settings in the order defined by `LintKind`.
    pub fn iter(&self) -> impl Iterator<Item = &LintConfig> {
        self.entries.values()
    }
}

impl FromIterator<LintConfig> for LintSettings {
    fn from_iter<T: IntoIterator<Item = LintConfig>>(iter: T) -> Self {
        let mut settings = Self::default();
        for config in iter {
            settings.insert(config);
        }
        settings
    }
}

impl Serialize for LintSettings {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(
            self.iter()
                .map(|config| (config.kind().name(), config.severity())),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn rule_names_and_setting_variants_correspond_exactly() {
        let mut names = BTreeSet::new();
        for &kind in LintKind::ALL {
            assert!(names.insert(kind.name()));
            assert_eq!(LintKind::from_name(kind.name()), Some(kind));
            let json = serde_json::to_value(kind).unwrap();
            assert_eq!(json, kind.name());
            assert_eq!(serde_json::from_value::<LintKind>(json).unwrap(), kind);
            for severity in [LintSeverity::Off, LintSeverity::Warn, LintSeverity::Deny] {
                let config = LintConfig::new(kind, severity);
                assert_eq!(config.kind(), kind);
                assert_eq!(config.severity(), severity);
            }
        }
        assert_eq!(names.len(), 32);
        for name in [
            "custom",
            "",
            "RequireRefAttachment",
            "require-ref-attachment ",
        ] {
            assert_eq!(LintKind::from_name(name), None);
            assert!(serde_json::from_value::<LintKind>(serde_json::json!(name)).is_err());
        }
    }

    #[test]
    fn defaults_and_overrides_preserve_reporting_and_detection_policy() {
        let mut settings = LintSettings::with_defaults();
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::json!({
                "framework-import-mismatch": "warn",
                "macro-argument-mutation": "deny",
                "macro-execution-boundary": "deny",
                "require-element-scope": "deny",
                "require-view-transition-name": "warn",
                "single-view-transition-name": "warn",
                "single-view-transition-class": "warn",
                "ambiguous-base-order": "warn",
                "imported-base-interference": "warn",
                "canonical-utilities": "warn",
                "duplicate-utilities": "warn",
                "conflicting-utilities": "warn",
                "valid-lint-suppressions": "warn",
            })
        );
        assert!(!settings.enabled_for_detection(LintKind::PreferTypedRelationships));
        assert_eq!(
            settings.reporting_severity(LintKind::PreferTypedRelationships),
            LintSeverity::Warn
        );
        assert_eq!(
            settings.insert(LintConfig::RequireElementScope(LintSeverity::Off)),
            Some(LintConfig::RequireElementScope(LintSeverity::Deny))
        );
        assert!(!settings.enabled_for_detection(LintKind::RequireElementScope));
        assert_eq!(
            settings.reporting_severity(LintKind::RequireElementScope),
            LintSeverity::Off
        );
        settings.insert(LintConfig::PreferTypedRelationships(LintSeverity::Deny));
        assert!(settings.enabled_for_detection(LintKind::PreferTypedRelationships));
        assert_eq!(
            settings.reporting_severity(LintKind::PreferTypedRelationships),
            LintSeverity::Deny
        );
        assert_eq!(
            serde_json::to_value(&settings).unwrap()["prefer-typed-relationships"],
            "deny"
        );
    }
}
