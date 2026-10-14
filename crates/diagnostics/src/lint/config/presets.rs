//! Built-in reporting policies shared by direct Rust callers and generated host presets.

use serde::{Deserialize, Serialize};

use super::{LintConfig, LintKind, LintSettings, LintSeverity};

const RECOMMENDED: &[(LintKind, LintSeverity)] = &[
    (LintKind::ValidLintSuppressions, LintSeverity::Warn),
    (LintKind::DuplicateUtilities, LintSeverity::Warn),
    (LintKind::ConflictingUtilities, LintSeverity::Warn),
    (LintKind::FrameworkImportMismatch, LintSeverity::Warn),
    (LintKind::MacroArgumentMutation, LintSeverity::Deny),
    (LintKind::MacroExecutionBoundary, LintSeverity::Deny),
    (LintKind::InvalidVariantInput, LintSeverity::Deny),
    (LintKind::ValidAnchorScope, LintSeverity::Warn),
    (LintKind::RequireRefAttachment, LintSeverity::Warn),
    (LintKind::ConflictingProviderProperties, LintSeverity::Warn),
    (LintKind::RequireElementScope, LintSeverity::Deny),
    (LintKind::RequireViewTransitionName, LintSeverity::Warn),
    (LintKind::SingleViewTransitionName, LintSeverity::Warn),
    (LintKind::SingleViewTransitionClass, LintSeverity::Warn),
    (LintKind::AmbiguousBaseOrder, LintSeverity::Warn),
    (LintKind::ImportedBaseInterference, LintSeverity::Warn),
    (LintKind::InvalidUtility, LintSeverity::Warn),
    (LintKind::UnknownUtility, LintSeverity::Warn),
    (LintKind::CanonicalUtilities, LintSeverity::Warn),
];

const STYLISTIC: &[(LintKind, LintSeverity)] = &[
    (LintKind::PreferDirectStyling, LintSeverity::Warn),
    (LintKind::PreferLogicalProperties, LintSeverity::Warn),
    (LintKind::PreferTypedRelationships, LintSeverity::Warn),
    (LintKind::SortUtilities, LintSeverity::Warn),
];

const STRICT_ADDITIONS: &[(LintKind, LintSeverity)] =
    &[(LintKind::NoUnusedLintSuppressions, LintSeverity::Deny)];

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
            strict.reporting_severity(LintKind::NoUnusedLintSuppressions),
            LintSeverity::Deny
        );
        let stylistic = LintPreset::Stylistic.settings();
        assert_eq!(
            stylistic.reporting_severity(LintKind::SortUtilities),
            LintSeverity::Warn
        );
        assert_eq!(
            stylistic.reporting_severity(LintKind::UnknownUtility),
            LintSeverity::Off
        );
    }
}
