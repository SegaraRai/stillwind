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
                $variant(#[doc = "Reporting severity; `Off` disables configured detection and reporting."] LintSeverity),
            )+
        }

        impl LintConfig {
            /// Constructs the setting variant corresponding to `kind` with `severity`.
            pub const fn new(kind: LintKind, severity: LintSeverity) -> Self {
                match kind { $(LintKind::$variant => Self::$variant(severity),)+ }
            }

            /// Returns the rule this setting configures.
            pub const fn kind(&self) -> LintKind {
                match self { $(Self::$variant(_) => LintKind::$variant,)+ }
            }

            /// Returns the configured reporting severity.
            pub const fn severity(&self) -> LintSeverity {
                match self { $(Self::$variant(severity) => *severity,)+ }
            }
        }
    };
}

define_lint_rules! {
    FrameworkImportMismatch => ("framework-import-mismatch", Some(LintSeverity::Warn)),
    MacroArgumentMutation => ("macro-argument-mutation", Some(LintSeverity::Deny)),
    MacroExecutionBoundary => ("macro-execution-boundary", Some(LintSeverity::Deny)),
    ValidAnchorScope => ("valid-anchor-scope", None),
    RequireRefAttachment => ("require-ref-attachment", None),
    ConflictingProviderProperties => ("conflicting-provider-properties", None),
    RequireAnchorProvider => ("require-anchor-provider", None),
    RequireContainerProvider => ("require-container-provider", None),
    ValidRefAncestry => ("valid-ref-ancestry", None),
    UnattachedRefScope => ("unattached-ref-scope", None),
    RefInScope => ("ref-in-scope", None),
    TargetInRefScope => ("target-in-ref-scope", None),
    PreferTypedRelationships => ("prefer-typed-relationships", None),
    RequireElementScope => ("require-element-scope", Some(LintSeverity::Deny)),
    RequireViewTransitionName => ("require-view-transition-name", Some(LintSeverity::Warn)),
    SingleViewTransitionName => ("single-view-transition-name", Some(LintSeverity::Warn)),
    SingleViewTransitionClass => ("single-view-transition-class", Some(LintSeverity::Warn)),
    AmbiguousBaseOrder => ("ambiguous-base-order", Some(LintSeverity::Warn)),
    ImportedBaseInterference => ("imported-base-interference", Some(LintSeverity::Warn)),
    RejectedIdentifier => ("rejected-identifier", None),
    InvalidUtility => ("invalid-utility", None),
    UnknownUtility => ("unknown-utility", None),
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
        assert_eq!(names.len(), 22);
        for name in [
            "custom",
            "",
            "RequireAnchorProvider",
            "require-anchor-provider ",
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
