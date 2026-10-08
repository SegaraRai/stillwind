//! Effective per-rule settings and separate detection and reporting lookups.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

use super::{LintConfig, LintKind, LintSeverity};

/// Settings indexed by rule, with at most one setting for each rule.
/// Absent settings return `false` from `enabled_for_detection`. Producers can detect
/// findings independently of that lookup; reporting an unconfigured rule uses `Warn`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintSettings {
    entries: BTreeMap<LintKind, LintConfig>,
}

impl LintSettings {
    /// Returns the compiler's default lint settings.
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

    /// Returns the effective setting for `kind`, if present.
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
