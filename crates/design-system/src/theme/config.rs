//! The `@stillwind` configuration block.
//!
//! Only settings the loader can resolve from CSS itself live here: utility ordering,
//! custom-property prefixes and lint severities. A key is stored as a lint rule only
//! when its value is `warn`, `deny`, or `off`, so an unrelated declaration in the same
//! block cannot silently become a policy.

use std::collections::BTreeMap;

use serde::Serialize;

use stillwind_css::{extract_named_blocks, strip_css_comments};
use stillwind_diagnostics::LintKind;

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
            } else if let Some(severity) = LintSeverity::from_css(value) {
                config.lint_rules.insert(key.to_owned(), severity);
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
    pub lint_rules: BTreeMap<String, LintSeverity>,
}

impl Default for StillwindConfig {
    fn default() -> Self {
        Self {
            utility_order: UtilityOrderMode::default(),
            prefix: None,
            tailwind_prefix: None,
            lint_rules: [
                (LintKind::MacroArgumentMutation, LintSeverity::Deny),
                (LintKind::MacroExecutionBoundary, LintSeverity::Deny),
                (
                    LintKind::DocumentScopedSelector {
                        selector: String::new(),
                    },
                    LintSeverity::Deny,
                ),
                (LintKind::MissingViewTransitionName, LintSeverity::Warn),
                (
                    LintKind::DuplicateViewTransitionName { count: 0 },
                    LintSeverity::Warn,
                ),
                (LintKind::SplitViewTransitionClasses, LintSeverity::Warn),
                (
                    LintKind::AmbiguousBaseOrder {
                        properties: String::new(),
                    },
                    LintSeverity::Warn,
                ),
                (
                    LintKind::ImportedBaseInterference {
                        module: String::new(),
                        properties: String::new(),
                    },
                    LintSeverity::Warn,
                ),
            ]
            .into_iter()
            .map(|(kind, level)| (kind.code().to_owned(), level))
            .collect(),
        }
    }
}

impl StillwindConfig {
    /// Whether a rule is active, which is the case for `warn` and `deny` and never for
    /// `off` or an unmentioned rule.
    #[must_use]
    pub fn lint_enabled(&self, rule: &str) -> bool {
        self.lint_rules
            .get(rule)
            .is_some_and(|level| matches!(level, LintSeverity::Warn | LintSeverity::Deny))
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

/// Configured handling of a lint finding by the consuming host.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    /// Reports a warning (`warn`).
    Warn,
    /// Reports an error (`deny`).
    Deny,
    /// Suppresses reporting (`off`).
    Off,
}

impl LintSeverity {
    /// The CSS configuration and host-protocol spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Off => "off",
        }
    }

    fn from_css(value: &str) -> Option<Self> {
        match value {
            "warn" => Some(Self::Warn),
            "deny" => Some(Self::Deny),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
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
             custom: deny; quiet: off; ignored: unknown; }",
        );
        assert_eq!(config.utility_order, UtilityOrderMode::Tailwind);
        assert_eq!(
            config.lint_rules["macro-argument-mutation"],
            LintSeverity::Off
        );
        assert_eq!(config.lint_rules["custom"], LintSeverity::Deny);
        assert!(!config.lint_rules.contains_key("ignored"));
        assert!(config.lint_enabled("custom"));
        assert!(!config.lint_enabled("quiet"));
        let config = extract_stillwind_config(
            "@stillwind { utility-order: tailwind; } @stillwind { utility-order: 'authored'; }",
        );
        assert_eq!(config.utility_order, UtilityOrderMode::Authored);
    }

    #[test]
    fn config_parsing_ignores_comments_and_distinguishes_disabled_rules() {
        let config = extract_stillwind_config(
            "/* @stillwind { prefix: nope; } */ @stillwind { prefix: 'app'; tailwind-prefix: tw; unknown: warn; quiet: off; }",
        );
        assert_eq!(config.prefix.as_deref(), Some("app"));
        assert_eq!(config.tailwind_prefix.as_deref(), Some("tw"));
        assert!(config.lint_enabled("unknown"));
        assert!(!config.lint_enabled("quiet"));
        assert!(!config.lint_enabled("absent"));
    }
}
