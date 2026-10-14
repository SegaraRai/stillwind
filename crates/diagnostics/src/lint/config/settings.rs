//! Effective per-rule settings and separate detection and reporting lookups.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

use super::{LintConfig, LintKind, LintPreset, LintSeverity};

/// Settings indexed by rule, with at most one setting for each rule.
/// Absent settings disable detection and reporting. Producers can still collect raw
/// findings independently of the detection lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintSettings {
    entries: BTreeMap<LintKind, LintConfig>,
}

impl LintSettings {
    /// Returns the recommended preset. Other rules remain unconfigured and report off.
    pub fn with_defaults() -> Self {
        LintPreset::Recommended.settings()
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

    /// Returns whether any configured rule enables detection.
    pub fn has_enabled_rules(&self) -> bool {
        self.entries
            .values()
            .any(|config| config.severity() != LintSeverity::Off)
    }

    /// Returns the configured severity, or `Off` for an unconfigured rule.
    pub fn reporting_severity(&self, kind: LintKind) -> LintSeverity {
        self.get(kind)
            .map_or(LintSeverity::Off, LintConfig::severity)
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
            LintKind::ALL
                .iter()
                .map(|&kind| (kind.name(), self.reporting_severity(kind))),
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
        assert_eq!(names.len(), 45);
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
            serde_json::from_str::<serde_json::Value>(
                r#"{
                "framework-import-mismatch": "warn",
                "macro-argument-mutation": "deny",
                "macro-execution-boundary": "deny",
                "invalid-variant-input": "deny",
                "invalid-animation-option": "deny",
                "invalid-cva-config": "deny",
                "invalid-cva-input": "deny",
                "invalid-keyframes-definition": "deny",
                "invalid-keyframes-use": "deny",
                "invalid-keyframes-variant": "deny",
                "invalid-ref-input": "deny",
                "invalid-style-composition": "deny",
                "invalid-style-input": "deny",
                "invalid-view-transition-definition": "deny",
                "invalid-view-transition-use": "deny",
                "invalid-view-transition-variant": "deny",
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
                "valid-anchor-scope": "warn",
                "require-ref-attachment": "warn",
                "conflicting-provider-properties": "warn",
                "invalid-utility": "warn",
                "unknown-utility": "warn",
                "no-arbitrary-colors": "off",
                "no-arbitrary-spacing": "off",
                "no-class-property": "off",
                "no-restricted-utilities": "off",
                "no-restricted-declarations": "off",
                "no-important": "off",
                "no-unused-lint-suppressions": "off",
                "prefer-direct-styling": "off",
                "prefer-logical-properties": "off",
                "length-units": "off",
                "no-restricted-theme-tokens": "off",
                "no-arbitrary-breakpoints": "off",
                "prefer-typed-relationships": "off",
                "sort-utilities": "off"
            }"#
            )
            .unwrap()
        );
        assert!(!settings.enabled_for_detection(LintKind::PreferTypedRelationships));
        assert_eq!(
            settings.reporting_severity(LintKind::PreferTypedRelationships),
            LintSeverity::Off
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
