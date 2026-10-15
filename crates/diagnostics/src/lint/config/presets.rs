//! Built-in reporting policies shared by direct Rust callers and generated host presets.

use serde::{Deserialize, Serialize};

use super::{LintConfig, LintKind, LintSettings, LintSeverity};

const RECOMMENDED: &[(LintKind, LintSeverity)] = &[
    (LintKind::InvalidLintSuppression, LintSeverity::Warn),
    (LintKind::DuplicateUtility, LintSeverity::Warn),
    (LintKind::ConflictingUtilities, LintSeverity::Warn),
    (LintKind::MismatchedFrameworkImport, LintSeverity::Warn),
    (LintKind::MutatingMacroArgument, LintSeverity::Deny),
    (LintKind::SuspendingMacroArgument, LintSeverity::Deny),
    (LintKind::InvalidVariantInput, LintSeverity::Deny),
    (LintKind::InvalidStyleInput, LintSeverity::Deny),
    (LintKind::InvalidCvaConfig, LintSeverity::Deny),
    (LintKind::InvalidCvaInput, LintSeverity::Deny),
    (LintKind::InvalidKeyframesUse, LintSeverity::Deny),
    (LintKind::InvalidKeyframesDefinition, LintSeverity::Deny),
    (LintKind::InvalidKeyframesVariant, LintSeverity::Deny),
    (LintKind::InvalidViewTransitionUse, LintSeverity::Deny),
    (
        LintKind::InvalidViewTransitionDefinition,
        LintSeverity::Deny,
    ),
    (LintKind::InvalidViewTransitionVariant, LintSeverity::Deny),
    (LintKind::InvalidRefInput, LintSeverity::Deny),
    (LintKind::InvalidAnimationOption, LintSeverity::Deny),
    (LintKind::InvalidStyleComposition, LintSeverity::Deny),
    (LintKind::RuntimeConditionRequiresCssIf, LintSeverity::Deny),
    (LintKind::UnverifiedStyleBase, LintSeverity::Warn),
    (LintKind::IneffectiveAnchorScope, LintSeverity::Warn),
    (LintKind::MissingRefAttachment, LintSeverity::Warn),
    (LintKind::ConflictingProviderProperties, LintSeverity::Warn),
    (LintKind::DocumentScopedSelector, LintSeverity::Deny),
    (LintKind::MissingViewTransitionName, LintSeverity::Warn),
    (LintKind::DuplicateViewTransitionName, LintSeverity::Warn),
    (LintKind::SplitViewTransitionClass, LintSeverity::Warn),
    (LintKind::AmbiguousStyleBaseOrder, LintSeverity::Warn),
    (LintKind::InterferingImportedStyleBase, LintSeverity::Warn),
    (LintKind::InvalidUtility, LintSeverity::Warn),
    (LintKind::UnknownUtility, LintSeverity::Warn),
    (LintKind::PreferCanonicalUtilities, LintSeverity::Warn),
];

const STYLISTIC: &[(LintKind, LintSeverity)] = &[
    (LintKind::PreferDirectStyling, LintSeverity::Warn),
    (LintKind::PreferLogicalProperties, LintSeverity::Warn),
    (LintKind::PreferTypedRelationships, LintSeverity::Warn),
    (LintKind::PreferSortedUtilities, LintSeverity::Warn),
];

const STRICT_ADDITIONS: &[(LintKind, LintSeverity)] =
    &[(LintKind::UnusedLintSuppression, LintSeverity::Deny)];

/// A built-in set of lint severities. Rule options use their catalog defaults.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LintPreset {
    /// Standard correctness checks and canonical utility suggestions.
    #[default]
    Recommended,
    /// Recommended checks and unused suppression checks, reported as errors.
    Strict,
    /// Optional conventions for relationships, declarations, and utility ordering.
    Stylistic,
}

impl LintPreset {
    /// Returns the configured rules. Rules outside this preset remain unconfigured and report off.
    pub fn settings(self) -> LintSettings {
        let mut settings = LintSettings::default();
        let entries = match self {
            Self::Recommended | Self::Strict => RECOMMENDED,
            Self::Stylistic => STYLISTIC,
        };
        for &(kind, severity) in entries {
            settings.insert(LintConfig::new(
                kind,
                if self == Self::Strict {
                    LintSeverity::Deny
                } else {
                    severity
                },
            ));
        }
        if self == Self::Strict {
            for &(kind, severity) in STRICT_ADDITIONS {
                settings.insert(LintConfig::new(kind, severity));
            }
        }
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::super::LintCategory;
    use super::*;

    #[test]
    fn presets_keep_project_restrictions_off_and_strict_denies_recommended() {
        for preset in [
            LintPreset::Recommended,
            LintPreset::Strict,
            LintPreset::Stylistic,
        ] {
            let settings = preset.settings();
            assert_eq!(
                settings.reporting_severity(LintKind::NoArbitraryColors),
                LintSeverity::Off
            );
            assert_eq!(
                settings.reporting_severity(LintKind::NoRestrictedThemeTokens),
                LintSeverity::Off
            );
        }
        let recommended = LintPreset::Recommended.settings();
        let strict = LintPreset::Strict.settings();
        for config in recommended.iter() {
            if config.severity() != LintSeverity::Off {
                assert_eq!(strict.reporting_severity(config.kind()), LintSeverity::Deny);
            }
        }
        assert_eq!(
            strict.reporting_severity(LintKind::UnusedLintSuppression),
            LintSeverity::Deny
        );
        let stylistic = LintPreset::Stylistic.settings();
        assert_eq!(
            stylistic.reporting_severity(LintKind::PreferSortedUtilities),
            LintSeverity::Warn
        );
        assert_eq!(
            stylistic.reporting_severity(LintKind::UnknownUtility),
            LintSeverity::Off
        );
    }

    #[test]
    fn presets_cover_rules_by_category() {
        let recommended = LintPreset::Recommended.settings();
        let stylistic = LintPreset::Stylistic.settings();
        for &kind in LintKind::ALL {
            let enabled =
                |settings: &LintSettings| settings.reporting_severity(kind) != LintSeverity::Off;
            match kind.category() {
                LintCategory::Input | LintCategory::Contract | LintCategory::Hazard => {
                    assert!(enabled(&recommended), "{}", kind.name());
                }
                LintCategory::Restriction => {
                    assert!(
                        !enabled(&recommended) && !enabled(&stylistic),
                        "{}",
                        kind.name()
                    );
                }
                LintCategory::Style => {
                    assert!(
                        enabled(&recommended) || enabled(&stylistic),
                        "{}",
                        kind.name()
                    );
                }
                LintCategory::Suppression => {}
            }
        }
    }
}
