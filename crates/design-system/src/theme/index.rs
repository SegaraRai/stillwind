use std::collections::{BTreeMap, BTreeSet};

use stillwind_css::{CssOutputTemplate, CustomPropertyDefinition, custom_property_references};

use super::config::StillwindConfig;
use super::declarations::{ThemeToken, ThemeTokenKind};
use super::namespace::ThemeNamespace;
use super::references::resolve_theme_references;
use crate::registry::utilities::CustomUtility;
use crate::registry::variants::CustomVariant;
use crate::{BuiltinDefinitions, ThemeTokenMode};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeIndex {
    pub output_template: CssOutputTemplate,
    pub registry: std::sync::Arc<BuiltinDefinitions>,
    pub tokens: Vec<ThemeToken>,
    pub custom_css: String,
    pub utilities: Vec<CustomUtility>,
    pub variants: Vec<CustomVariant>,
    pub properties: Vec<CustomPropertyDefinition>,
    /// Utility names whose declarations provide compatibility marker classes.
    pub marker_utilities: BTreeSet<String>,
    /// Generic serialization formats attached to CSS declarations.
    pub declaration_formats: BTreeMap<String, String>,
    pub relationship_patterns: BTreeSet<String>,
    pub authored_properties: BTreeSet<String>,
    pub config: StillwindConfig,
}

impl ThemeIndex {
    #[must_use]
    pub fn token_in(&self, namespace: ThemeNamespace, name: &str) -> Option<&ThemeToken> {
        self.active_tokens().into_iter().find(|token| {
            token.kind
                == (ThemeTokenKind::Token {
                    namespace,
                    name: name.to_owned(),
                })
        })
    }

    #[must_use]
    pub fn tokens_in(&self, namespace: ThemeNamespace) -> Vec<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .filter(|token| {
                matches!(
                    token.kind,
                    ThemeTokenKind::Token {
                        namespace: token_namespace,
                        ..
                    } if token_namespace == namespace
                )
            })
            .collect()
    }

    #[must_use]
    pub fn scale(&self, namespace: ThemeNamespace) -> Option<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .find(|token| token.kind == ThemeTokenKind::Scale(namespace))
    }

    #[must_use]
    pub fn token(&self, namespace: &str, name: &str) -> Option<&ThemeToken> {
        if let Some(namespace) = ThemeNamespace::parse(namespace) {
            return self.token_in(namespace, name);
        }
        let expected = format!("--{namespace}-{name}");
        self.active_tokens()
            .into_iter()
            .find(|token| token.name.value == expected)
    }

    #[must_use]
    pub fn value(&self, namespace: &str, name: &str) -> Option<&str> {
        self.token(namespace, name)
            .map(|token| token.value.value.as_str())
    }

    #[must_use]
    pub fn custom_property(&self, name: &str) -> Option<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .find(|token| token.name.value == name)
    }

    /// Lowers build-time `--theme()` references in a theme-token value.
    ///
    /// An available token becomes a standard `var()` reference. An unavailable
    /// token uses its authored fallback; `initial` is the sentinel Tailwind
    /// uses to omit the containing alias altogether.
    #[must_use]
    pub fn resolve_theme_references(&self, value: &str) -> Option<String> {
        resolve_theme_references(value, |name| self.custom_property(name).is_some())
    }

    /// Returns the effective CSS custom properties after v4 namespace resets.
    #[must_use]
    pub fn active_tokens(&self) -> Vec<&ThemeToken> {
        let mut values = BTreeMap::new();
        let mut clear_all = false;
        for token in &self.tokens {
            if token.name.value == "--*" && token.value.value == "initial" {
                values.clear();
                clear_all = true;
                continue;
            }
            if let Some(namespace) = token
                .name
                .value
                .strip_prefix("--")
                .and_then(|value| value.strip_suffix("-*"))
                && token.value.value == "initial"
            {
                values.retain(|name: &String, _| !name.starts_with(&format!("--{namespace}-")));
                continue;
            }
            if clear_all || token.name.value.starts_with("--") {
                values.insert(token.name.value.clone(), token);
            }
        }
        values.into_values().collect()
    }

    /// Selects the effective tokens required by emitted declarations. Project
    /// tokens are retained as authored; vendored defaults are tree-shaken and
    /// their `var()` dependencies are followed transitively.
    #[must_use]
    pub fn tokens_required_by(&self, css: &str, include_preflight: bool) -> Vec<&ThemeToken> {
        self.tokens_required_by_with_omit_rules(css, include_preflight, &[])
    }

    /// Selects required tokens while applying registry-authored pruning rules.
    #[must_use]
    pub fn tokens_required_by_with_omit_rules(
        &self,
        css: &str,
        include_preflight: bool,
        omit_rules: &[(String, String)],
    ) -> Vec<&ThemeToken> {
        let tokens = self.active_tokens();
        let mut required = custom_property_references(css);
        if include_preflight {
            required.extend(custom_property_references(&self.custom_css));
        }
        for token in &tokens {
            if matches!(token.mode, ThemeTokenMode::Static)
                || (!matches!(token.mode, ThemeTokenMode::Inline)
                    && custom_property_references(css).contains(&token.name.value))
            {
                required.insert(token.name.value.clone());
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for token in &tokens {
                if required.contains(&token.name.value) {
                    for dependency in custom_property_references(&token.value.value) {
                        changed |= required.insert(dependency);
                    }
                }
            }
        }
        // Registry-authored pruning rules can remove intermediate aliases
        // after dependency closure, without embedding property names here.
        for (prefix, condition) in omit_rules {
            if css.contains(condition) {
                required.retain(|name| !name.starts_with(prefix));
            }
        }
        tokens
            .into_iter()
            .filter(|token| {
                required.contains(&token.name.value)
                    && !matches!(token.mode, ThemeTokenMode::Inline)
            })
            .collect()
    }

    /// CSS outside of `@theme` blocks, retained verbatim for custom properties,
    /// theme selectors, layers, and ordinary authored CSS.
    #[must_use]
    pub fn custom_css(&self) -> &str {
        &self.custom_css
    }

    #[must_use]
    pub fn utility(&self, name: &str) -> Option<&CustomUtility> {
        self.utilities
            .iter()
            .rev()
            .find(|utility| utility.name == name)
    }

    #[must_use]
    pub fn variant(&self, name: &str) -> Option<&CustomVariant> {
        self.variants
            .iter()
            .rev()
            .find(|variant| variant.name == name)
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;
    use crate::theme::declarations::extract_theme_declarations;

    #[test]
    fn resets_and_redefinitions_select_only_effective_tokens() {
        let theme = ThemeIndex {
            tokens: extract_theme_declarations(
                FileId(1),
                "--color-old:red;--color-*:initial;--color-new:blue;--color-new:green;",
                0,
                ThemeTokenMode::Default,
            )
            .unwrap(),
            ..ThemeIndex::default()
        };
        assert_eq!(theme.active_tokens().len(), 1);
        assert_eq!(theme.value("color", "old"), None);
        assert_eq!(theme.value("color", "new"), Some("green"));
        assert_eq!(theme.tokens_in(ThemeNamespace::Color).len(), 1);
        assert_eq!(
            theme
                .resolve_theme_references("--theme(--color-new)")
                .as_deref(),
            Some("var(--color-new)")
        );
    }

    #[test]
    fn transitive_token_dependencies_terminate_even_on_cycles() {
        let theme = ThemeIndex {
            tokens: extract_theme_declarations(
                FileId(1),
                "--color-a:var(--color-b);--color-b:var(--color-a);--color-unused:red;",
                0,
                ThemeTokenMode::Default,
            )
            .unwrap(),
            ..ThemeIndex::default()
        };
        let names = theme
            .tokens_required_by("color:var(--color-a)", false)
            .iter()
            .map(|t| t.name.value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["--color-a", "--color-b"]);
    }
}
