//! The `@stillwind` configuration block.
//!
//! Only settings the loader can resolve from CSS itself live here: utility ordering,
//! custom-property prefixes and lint severities. A key is stored as a lint rule only
//! when its name identifies a supported rule and its value is `warn`, `deny`, or `off`.

use stillwind_css::{extract_named_blocks, strip_css_comments};
use stillwind_diagnostics::{LintConfig, LintKind, LintSettings, LintSeverity};

/// Extracts the configuration from every `@stillwind` block, after comments are removed
/// so a commented example cannot configure a project.
///
/// Declarations are read in authored order and the last occurrence of a key wins.
/// Quoted values are accepted, and only the keys this parser knows are kept.
pub(crate) fn extract_stillwind_config(css: &str) -> StillwindConfig {
    let mut config = StillwindConfig::default();
    for (_, body) in extract_named_blocks(&strip_css_comments(css), "@stillwind") {
        for declaration in body.split(';') {
            let declaration = declaration.trim();
            let Some((key, value)) = declaration.split_once(':') else {
                continue;
            };
            let key = key
                .rsplit_once('{')
                .map_or(key.trim(), |(_, nested)| nested.trim());
            let value = value.trim().trim_matches(['"', '\'']);
            if key == "prefix" {
                config.prefix = Some(value.to_owned());
            } else if key == "tailwind-prefix" {
                config.tailwind_prefix = Some(value.to_owned());
            } else if key == "utility-order" {
                match value {
                    "authored" => config.utility_order = UtilityOrderMode::Authored,
                    "tailwind" => config.utility_order = UtilityOrderMode::Tailwind,
                    _ => {}
                }
            } else if let Some(kind) = LintKind::from_name(key)
                && let Some(severity) = LintSeverity::from_css(value)
            {
                config.lint_settings.insert(LintConfig::new(kind, severity));
            }
        }
    }
    config
}

/// CSS configuration with effective lint severities, including rule defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StillwindConfig {
    /// Ordering strategy for resolved utilities; defaults to definition positions.
    pub utility_order: UtilityOrderMode,
    /// Stillwind-owned prefix, when the CSS configures one.
    pub prefix: Option<String>,
    /// Tailwind-compatibility prefix, when the CSS configures one.
    pub tailwind_prefix: Option<String>,
    /// Rule defaults overridden by CSS-authored severities.
    pub lint_settings: LintSettings,
}

impl Default for StillwindConfig {
    fn default() -> Self {
        Self {
            utility_order: UtilityOrderMode::default(),
            prefix: None,
            tailwind_prefix: None,
            lint_settings: LintSettings::with_defaults(),
        }
    }
}

/// CSS-configured strategy for ordering resolved utilities.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UtilityOrderMode {
    /// First definition positions in the import-expanded stylesheet (`authored`).
    #[default]
    Authored,
    /// Tailwind property metadata and candidate comparison (`tailwind`).
    Tailwind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_keeps_valid_overrides_and_ignores_unknown_values() {
        assert_eq!(
            StillwindConfig::default().utility_order,
            UtilityOrderMode::Authored
        );
        let config = extract_stillwind_config(
            "@stillwind { utility-order: tailwind; utility-order: unknown;
             macro-argument-mutation: off; macro-argument-mutation: unknown;
             valid-anchor-scope: deny; unknown-utility: off;
             custom: deny; quiet: off; ignored: unknown; }",
        );
        assert_eq!(config.utility_order, UtilityOrderMode::Tailwind);
        assert_eq!(
            config
                .lint_settings
                .reporting_severity(LintKind::MacroArgumentMutation),
            LintSeverity::Off
        );
        assert_eq!(
            config.lint_settings.get(LintKind::ValidAnchorScope),
            Some(&LintConfig::ValidAnchorScope(LintSeverity::Deny))
        );
        assert_eq!(
            config.lint_settings.iter().count(),
            LintSettings::with_defaults().iter().count() + 2
        );
        assert!(
            !config
                .lint_settings
                .enabled_for_detection(LintKind::UnknownUtility)
        );
        assert!(
            !config
                .lint_settings
                .enabled_for_detection(LintKind::InvalidUtility)
        );
        let config = extract_stillwind_config(
            "@stillwind { utility-order: tailwind; } @stillwind { utility-order: 'authored'; }",
        );
        assert_eq!(config.utility_order, UtilityOrderMode::Authored);
    }

    #[test]
    fn config_parsing_ignores_comments_and_distinguishes_disabled_rules() {
        let config = extract_stillwind_config(
            "/* @stillwind { prefix: nope; require-element-scope: off; } */
             @stillwind { prefix: 'app'; tailwind-prefix: tw;
             invalid-utility: 'off'; prefer-typed-relationships: warn;
             unknown: warn; quiet: off; }
             @stillwind { prefer-typed-relationships: 'deny'; }",
        );
        assert_eq!(config.prefix.as_deref(), Some("app"));
        assert_eq!(config.tailwind_prefix.as_deref(), Some("tw"));
        assert_eq!(
            config.lint_settings.iter().count(),
            LintSettings::with_defaults().iter().count() + 2
        );
        assert_eq!(
            config
                .lint_settings
                .reporting_severity(LintKind::RequireElementScope),
            LintSeverity::Deny
        );
        assert!(
            !config
                .lint_settings
                .enabled_for_detection(LintKind::InvalidUtility)
        );
        assert!(
            config
                .lint_settings
                .enabled_for_detection(LintKind::PreferTypedRelationships)
        );
        assert_eq!(
            config
                .lint_settings
                .reporting_severity(LintKind::PreferTypedRelationships),
            LintSeverity::Deny
        );
    }
}
