//! The parsed theme and its token queries.
//!
//! The index holds exactly what the supplied CSS declared, in authored order, with the
//! output template and the selected built-in definitions alongside it. Queries that
//! answer "what is active" or "what is required" compute their answer from the token
//! list instead of maintaining a second index, so a reset, a redefinition, and a
//! dependency all stay consistent with the authored order.
//!
//! [`ThemeIndex::active_tokens`] is the single definition of token effectiveness: a
//! later declaration replaces an earlier one, and a namespace reset removes the tokens
//! it covers. Every other lookup goes through it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_css::{CssOutputTemplate, CustomPropertyDefinition, custom_property_references};

use super::config::StillwindConfig;
use super::declarations::{ThemeToken, ThemeTokenKind};
use super::namespace::ThemeNamespace;
use super::references::resolve_theme_references;
use crate::registry::utilities::CustomUtility;
use crate::registry::variants::CustomVariant;
use crate::{BuiltinDefinitions, ThemeReferenceError, ThemeTokenMode};

/// Everything one parsed CSS buffer declared, before it is resolved into a snapshot.
///
/// The fields are the authored data, not a resolved lookup view: `tokens` keeps
/// superseded declarations and resets, so queries must go through the methods below
/// rather than reading the vector directly.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeIndex {
    /// First declaration offsets after imports are expanded, shared by user and built-in variants.
    pub variant_positions: BTreeMap<String, usize>,
    /// First declaration offsets after imports are expanded, shared by user and built-in utilities.
    pub utility_positions: BTreeMap<String, usize>,
    /// Structured output positions from the authored entry CSS.
    pub output_template: CssOutputTemplate,
    /// Built-in definitions selected by this CSS; empty when it imports none.
    pub registry: Arc<BuiltinDefinitions>,
    /// Every token in authored order, including resets and superseded values.
    pub tokens: Vec<ThemeToken>,
    /// Authored CSS outside definition blocks, retained verbatim.
    pub custom_css: String,
    /// User `@utility` definitions in authored order.
    pub utilities: Vec<CustomUtility>,
    /// User variant definitions grouped by syntax form; source order within each form.
    pub variants: Vec<CustomVariant>,
    /// Compiler-owned `@stillwind-property` definitions in authored order.
    pub properties: Vec<CustomPropertyDefinition>,
    /// Utility names whose declarations provide compatibility marker classes.
    pub marker_utilities: BTreeSet<String>,
    /// Generic serialization formats attached to CSS declarations.
    pub declaration_formats: BTreeMap<String, String>,
    /// Candidate patterns identifying compatibility relationships, such as `group-*`.
    pub relationship_patterns: BTreeSet<String>,
    /// Property names declared by authored `@property` rules.
    pub authored_properties: BTreeSet<String>,
    /// Resolved `@stillwind-config` values for this stylesheet.
    pub config: StillwindConfig,
}

impl ThemeIndex {
    /// The named token in `namespace`, or `None` when it is not active.
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

    /// Every active named token in `namespace`, ordered by custom-property name.
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

    /// The namespace's scalar scale token, when one is active.
    #[must_use]
    pub fn scale(&self, namespace: ThemeNamespace) -> Option<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .find(|token| token.kind == ThemeTokenKind::Scale(namespace))
    }

    /// Looks up a token by authored namespace text and bare name.
    ///
    /// A namespace that is not a known theme namespace falls back to treating the
    /// arguments as a custom-property name, so callers may query an arbitrary property
    /// without knowing whether it participates in a namespace.
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

    /// The authored value of a token, looked up as [`Self::token`] does.
    #[must_use]
    pub fn value(&self, namespace: &str, name: &str) -> Option<&str> {
        self.token(namespace, name)
            .map(|token| token.value.value.as_str())
    }

    /// The active token with exactly this custom-property name, if any.
    #[must_use]
    pub fn custom_property(&self, name: &str) -> Option<&ThemeToken> {
        self.active_tokens()
            .into_iter()
            .find(|token| token.name.value == name)
    }

    /// Lowers build-time `--theme()` references in a theme-token value.
    ///
    /// An available token becomes a standard `var()` reference. An unavailable token
    /// uses its authored fallback; `initial` is the sentinel Tailwind uses to omit the
    /// containing alias altogether.
    ///
    /// `Ok(None)` means the containing declaration must not be emitted at all.
    ///
    /// # Errors
    ///
    /// Returns malformed references with their byte offset within `value`.
    pub fn resolve_theme_references(
        &self,
        value: &str,
    ) -> Result<Option<String>, ThemeReferenceError> {
        resolve_theme_references(value, |name| self.custom_property(name).is_some())
    }

    /// Returns the effective CSS custom properties after v4 namespace resets.
    ///
    /// Resets and replacements are applied in authored order. The result is sorted
    /// by custom-property name because the effective values are stored in a `BTreeMap`.
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

    /// Selects tokens referenced by CSS and their transitive dependencies, then applies
    /// registry-authored pruning rules. Static tokens seed the selection, while inline
    /// tokens are never emitted.
    ///
    /// Pruning runs after dependency closure, so a rule may drop an intermediate alias
    /// without changing which declarations were considered.
    #[must_use]
    pub fn tokens_required_by(
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
        // Registry-authored pruning rules can remove intermediate aliases after
        // dependency closure, without embedding property names here.
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

    /// CSS outside of `@theme` blocks, retained verbatim for custom properties, theme
    /// selectors, layers, and ordinary authored CSS.
    #[must_use]
    pub fn custom_css(&self) -> &str {
        &self.custom_css
    }

    /// The last authored user utility with this exact name.
    ///
    /// Wildcard definitions are not matched here; candidate resolution owns
    /// prefix matching.
    #[must_use]
    pub fn utility(&self, name: &str) -> Option<&CustomUtility> {
        self.utilities
            .iter()
            .rev()
            .find(|utility| utility.name == name)
    }

    /// The last authored user variant with this exact name.
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
    use stillwind_source::{CssSourceName, FileId, SourceFile};

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
                .unwrap()
                .as_deref(),
            Some("var(--color-new)")
        );
        // Reset then redefinition keeps the last value, in name order.
        let ordered = ThemeIndex {
            tokens: extract_theme_declarations(
                FileId(1),
                "--color-z:red;--color-a:blue;--color-*:initial;--color-z:green;--color-a:black;",
                0,
                ThemeTokenMode::Default,
            )
            .unwrap(),
            ..ThemeIndex::default()
        };
        let tokens = ordered.active_tokens();
        assert_eq!(
            tokens
                .iter()
                .map(|token| token.name.value.as_str())
                .collect::<Vec<_>>(),
            ["--color-a", "--color-z"]
        );
        assert_eq!(tokens[1].value.value, "green");
    }

    /// Only tokens a declaration actually references survive pruning; `static` tokens
    /// are emitted whether or not anything uses them.
    #[test]
    fn ordinary_project_tokens_are_pruned_but_static_tokens_are_retained() {
        let theme = crate::parse_theme(
            &SourceFile::new(
                FileId(1),
                None,
                None,
                "@theme { --color-unused: red; } @theme static { --color-static: blue; }",
            )
            .unwrap(),
            &CssSourceName::from("test.css"),
        )
        .unwrap();
        assert_eq!(
            theme
                .tokens_required_by("", false, &[])
                .iter()
                .map(|token| token.name.value.as_str())
                .collect::<Vec<_>>(),
            ["--color-static"]
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
            .tokens_required_by("color:var(--color-a)", false, &[])
            .iter()
            .map(|t| t.name.value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["--color-a", "--color-b"]);
    }
}
