use std::collections::BTreeMap;

use crate::blocks::{extract_named_blocks, strip_css_comments};

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StillwindConfig {
    pub prefix: Option<String>,
    pub tailwind_prefix: Option<String>,
    pub lint_rules: BTreeMap<String, String>,
}

impl StillwindConfig {
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
