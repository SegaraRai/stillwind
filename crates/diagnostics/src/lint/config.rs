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
    (CanonicalUtilities) => {
        CanonicalUtilitiesConfig
    };
    ($variant:ident) => {
        LintSeverity
    };
}
macro_rules! lint_config_value {
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

/// Project overrides applied after defaults and CSS severity directives.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct LintOverrides {
    /// Field-wise canonical policy overrides.
    pub canonical_utilities: Option<CanonicalUtilitiesOverride>,
    /// Optional unsorted-source diagnostic. It is disabled by default.
    pub sort_utilities: Option<LintSeverity>,
}

impl LintOverrides {
    /// Resolves project fields against the stylesheet settings without changing either input.
    pub fn resolve(&self, stylesheet: &LintSettings) -> LintSettings {
        let mut settings = stylesheet.clone();
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
}

define_lint_rules! {
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
        assert_eq!(names.len(), 17);
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
