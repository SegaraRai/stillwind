//! The `@stillwind` configuration block.
//!
//! Only the settings the loader can resolve from CSS itself live here: the two
//! custom-property prefixes and lint severities. A key is stored as a lint rule only
//! when its value is `warn`, `deny`, or `off`, so an unrelated declaration in the same
//! block cannot silently become a policy.

use std::collections::BTreeMap;

use stillwind_css::{extract_named_blocks, strip_css_comments};

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
            } else if matches!(value, "warn" | "deny" | "off") {
                config.lint_rules.insert(key.to_owned(), value.to_owned());
            }
        }
    }
    config
}

/// CSS-authored configuration before it is resolved against host defaults.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StillwindConfig {
    /// Stillwind-owned prefix, when the CSS configures one.
    pub prefix: Option<String>,
    /// Tailwind-compatibility prefix, when the CSS configures one.
    pub tailwind_prefix: Option<String>,
    /// Severities for lint rules named by the CSS.
    pub lint_rules: BTreeMap<String, String>,
}

impl StillwindConfig {
    /// Whether a rule is active, which is the case for `warn` and `deny` and never for
    /// `off` or an unmentioned rule.
    #[must_use]
    pub fn lint_enabled(&self, rule: &str) -> bool {
        self.lint_rules
            .get(rule)
            .is_some_and(|level| matches!(level.as_str(), "warn" | "deny"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
