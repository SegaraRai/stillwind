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
        assert_eq!(names.len(), 68);
        for name in [
            "custom",
            "",
            "RequireRefAttachment",
            "missing-ref-attachment ",
        ] {
            assert_eq!(LintKind::from_name(name), None);
            assert!(serde_json::from_value::<LintKind>(serde_json::json!(name)).is_err());
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // The expected defaults list every rule once.
    fn defaults_and_overrides_preserve_reporting_and_detection_policy() {
        let mut settings = LintSettings::with_defaults();
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::from_str::<serde_json::Value>(
                r#"{
                "mismatched-framework-import": "warn",
                "mutating-macro-argument": "deny",
                "suspending-macro-argument": "deny",
                "invalid-rule-group-input": "deny",
                "invalid-animation-option": "deny",
                "invalid-cva-config": "deny",
                "invalid-cva-input": "deny",
                "invalid-keyframes-definition": "deny",
                "invalid-keyframes-use": "deny",
                "invalid-keyframes-variant": "deny",
                "invalid-ref-input": "deny",
                "invalid-style-composition": "deny",
                "runtime-condition-requires-css-if": "deny",
                "unverified-style-base": "warn",
                "invalid-style-input": "deny",
                "invalid-view-transition-definition": "deny",
                "invalid-view-transition-use": "deny",
                "invalid-view-transition-variant": "deny",
                "invalid-media-query": "deny",
                "invalid-css-input": "deny",
                "document-scoped-selector": "deny",
                "repeated-nesting-selector": "warn",
                "missing-view-transition-name": "warn",
                "duplicate-view-transition-name": "warn",
                "split-view-transition-class": "warn",
                "ambiguous-style-base-order": "warn",
                "interfering-imported-style-base": "warn",
                "prefer-canonical-utilities": "warn",
                "duplicate-utility": "warn",
                "conflicting-utilities": "warn",
                "invalid-lint-suppression": "warn",
                "ineffective-anchor-scope": "warn",
                "missing-ref-attachment": "warn",
                "conflicting-provider-properties": "warn",
                "invalid-utility": "deny",
                "unknown-utility": "warn",
                "invalid-marker-use": "warn",
                "invalid-variant": "deny",
                "unknown-variant": "warn",
                "missing-document-target": "deny",
                "invalid-property-value": "deny",
                "redundant-variant": "warn",
                "unsatisfiable-variants": "warn",
                "clamped-value": "warn",
                "invalid-theme-reference": "deny",
                "invalid-css-import": "deny",
                "invalid-css-directive": "deny",
                "duplicate-definition": "warn",
                "bypassed-undefinition": "warn",
                "invalid-theme-value": "deny",
                "ignored-tailwind-feature": "warn",
                "implicit-nesting-selector": "warn",
                "prefer-typed-variables": "off",
                "no-arbitrary-colors": "off",
                "no-arbitrary-spacing": "off",
                "no-class-property": "off",
                "no-restricted-utilities": "off",
                "no-restricted-declarations": "off",
                "no-important": "off",
                "unused-lint-suppression": "off",
                "prefer-direct-styling": "off",
                "prefer-logical-properties": "off",
                "no-restricted-length-units": "off",
                "no-restricted-theme-tokens": "off",
                "no-arbitrary-breakpoints": "off",
                "prefer-typed-relationships": "off",
                "prefer-sorted-utilities": "off",
                "prefer-variant-directive": "off"
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
            settings.insert(LintConfig::DocumentScopedSelector(LintSeverity::Off)),
            Some(LintConfig::DocumentScopedSelector(LintSeverity::Deny))
        );
        assert!(!settings.enabled_for_detection(LintKind::DocumentScopedSelector));
        assert_eq!(
            settings.reporting_severity(LintKind::DocumentScopedSelector),
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
