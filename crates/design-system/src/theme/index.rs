//! Immutable authored definitions and shared evaluation data for one CSS revision.
//!
//! Token lookup applies resets and replacements once. Clones share declarations,
//! effective token positions, candidate lookup, and parsed utility programs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{CssOutputTemplate, CustomPropertyDefinition, custom_property_references};

use super::builder::ThemeBuilder;
use super::declarations::ThemeToken;
use super::namespace::{ThemeNamespace, excluded_from_namespace, namespace_key_names};
use super::references::{inline_theme_functions, resolve_theme_references};
use super::tokens::TokenLookup;
use crate::evaluation::{UtilityPrograms, color_fallback};
use crate::registry::UtilityLookup;
use crate::{
    BrowserTargets, BuiltinDefinitions, CustomUtility, CustomVariant, ThemeReferenceError,
    ThemeTokenMode, UtilityOrderMode,
};

#[derive(Debug, Eq, PartialEq)]
struct ThemeDeclarations {
    variant_positions: BTreeMap<LeanStr, usize>,
    utility_positions: BTreeMap<LeanStr, usize>,
    output_template: CssOutputTemplate,
    css_templates: BTreeMap<LeanStr, CssOutputTemplate>,
    registry: Arc<BuiltinDefinitions>,
    tokens: Vec<Arc<ThemeToken>>,
    custom_css: LeanStr,
    utilities: Vec<Arc<CustomUtility>>,
    variants: Vec<Arc<CustomVariant>>,
    properties: Vec<Arc<CustomPropertyDefinition>>,
    marker_utilities: BTreeSet<LeanStr>,
    declaration_formats: BTreeMap<LeanStr, LeanStr>,
    relationship_patterns: BTreeSet<LeanStr>,
    authored_properties: BTreeSet<LeanStr>,
    authored_property_syntaxes: BTreeMap<LeanStr, LeanStr>,
    utility_order: UtilityOrderMode,
    target: BrowserTargets,
}

#[derive(Debug)]
struct ThemeData {
    declarations: ThemeDeclarations,
    tokens: TokenLookup,
    utilities: UtilityLookup,
    programs: OnceLock<UtilityPrograms>,
}

/// Owns one immutable stylesheet's authored declarations and evaluation data.
/// Clones share the same definitions and preparation. Parsing new CSS creates a new index.
#[derive(Clone, Debug)]
pub struct ThemeIndex {
    inner: Arc<ThemeData>,
}

impl Default for ThemeIndex {
    fn default() -> Self {
        ThemeBuilder::default().finish()
    }
}

impl PartialEq for ThemeIndex {
    fn eq(&self, other: &Self) -> bool {
        self.inner.declarations == other.inner.declarations
    }
}

impl Eq for ThemeIndex {}

impl ThemeIndex {
    pub(crate) fn new(builder: ThemeBuilder) -> Self {
        let declarations = ThemeDeclarations {
            variant_positions: builder.variant_positions,
            utility_positions: builder.utility_positions,
            output_template: builder.output_template,
            css_templates: builder.css_templates,
            registry: builder.registry,
            tokens: builder.tokens.into_iter().map(Arc::new).collect(),
            custom_css: builder.custom_css,
            utilities: builder.utilities.into_iter().map(Arc::new).collect(),
            variants: builder.variants.into_iter().map(Arc::new).collect(),
            properties: builder.properties.into_iter().map(Arc::new).collect(),
            marker_utilities: builder.marker_utilities,
            declaration_formats: builder.declaration_formats,
            relationship_patterns: builder.relationship_patterns,
            authored_properties: builder.authored_properties,
            authored_property_syntaxes: builder.authored_property_syntaxes,
            utility_order: builder.utility_order,
            target: builder.target,
        };
        let tokens = TokenLookup::new(&declarations.tokens);
        let utilities = UtilityLookup::new(&declarations.utilities);
        let theme = Self {
            inner: Arc::new(ThemeData {
                declarations,
                tokens,
                utilities,
                programs: OnceLock::new(),
            }),
        };
        theme.programs();
        theme
    }

    pub(crate) fn programs(&self) -> &UtilityPrograms {
        self.inner
            .programs
            .get_or_init(|| UtilityPrograms::new(self))
    }

    pub(crate) fn matching_utilities(&self, name: &str) -> Vec<&CustomUtility> {
        self.inner.utilities.matching(name, self.utilities(), false)
    }

    pub(crate) fn active_token_definitions(&self) -> impl Iterator<Item = &Arc<ThemeToken>> {
        self.inner
            .tokens
            .active
            .values()
            .map(|index| &self.inner.declarations.tokens[*index])
    }

    /// First declaration offsets after imports are expanded, shared by user and
    /// built-in variants.
    pub fn variant_positions(&self) -> &BTreeMap<LeanStr, usize> {
        &self.inner.declarations.variant_positions
    }

    /// First declaration offsets after imports are expanded, shared by user and
    /// built-in utilities.
    pub fn utility_positions(&self) -> &BTreeMap<LeanStr, usize> {
        &self.inner.declarations.utility_positions
    }

    /// Structured output positions from the authored entry CSS.
    pub fn output_template(&self) -> &CssOutputTemplate {
        &self.inner.declarations.output_template
    }

    /// Named output templates for `style.css()`, keyed by name. A later definition of a
    /// name replaced earlier ones.
    pub fn css_templates(&self) -> &BTreeMap<LeanStr, CssOutputTemplate> {
        &self.inner.declarations.css_templates
    }

    /// Built-in definitions selected by this CSS; empty when it imports none.
    pub fn registry(&self) -> &Arc<BuiltinDefinitions> {
        &self.inner.declarations.registry
    }

    /// Every token in authored order, including resets and superseded values.
    pub fn tokens(&self) -> &[Arc<ThemeToken>] {
        &self.inner.declarations.tokens
    }

    /// User `@utility` definitions in authored order.
    pub fn utilities(&self) -> &[Arc<CustomUtility>] {
        &self.inner.declarations.utilities
    }

    /// User variant definitions grouped by syntax form; source order within each form.
    pub fn variants(&self) -> &[Arc<CustomVariant>] {
        &self.inner.declarations.variants
    }

    /// Compiler-owned `@stillwind-property` definitions in authored order.
    pub fn properties(&self) -> &[Arc<CustomPropertyDefinition>] {
        &self.inner.declarations.properties
    }

    /// Utility names whose declarations provide compatibility marker classes.
    pub fn marker_utilities(&self) -> &BTreeSet<LeanStr> {
        &self.inner.declarations.marker_utilities
    }

    /// Generic serialization formats attached to CSS declarations.
    pub fn declaration_formats(&self) -> &BTreeMap<LeanStr, LeanStr> {
        &self.inner.declarations.declaration_formats
    }

    /// Candidate patterns identifying compatibility relationships, such as `group-*`.
    pub fn relationship_patterns(&self) -> &BTreeSet<LeanStr> {
        &self.inner.declarations.relationship_patterns
    }

    /// Property names declared by authored `@property` rules.
    pub fn authored_properties(&self) -> &BTreeSet<LeanStr> {
        &self.inner.declarations.authored_properties
    }

    /// The `syntax` descriptor of the authored `@property` rule for `name`, without
    /// quotes, such as `<length>`.
    #[must_use]
    pub fn authored_property_syntax(&self, name: &str) -> Option<&str> {
        self.inner
            .declarations
            .authored_property_syntaxes
            .get(name)
            .map(LeanStr::as_str)
    }

    /// Host-selected ordering strategy for resolved utilities.
    pub fn utility_order(&self) -> UtilityOrderMode {
        self.inner.declarations.utility_order
    }

    /// Browser versions captured for color compatibility output.
    pub(crate) fn target(&self) -> &BrowserTargets {
        &self.inner.declarations.target
    }

    /// Computes the compatibility declaration for a property/value pair.
    /// Known theme values are expanded in the fallback. Returns `None` when the
    /// declaration needs no fallback or every configured target supports its colors.
    pub fn color_fallback(&self, declaration: &str) -> Option<LeanString> {
        color_fallback(declaration, self)
    }

    /// The named token in `namespace`, or `None` when it is not active.
    #[must_use]
    pub fn token_in(&self, namespace: ThemeNamespace, name: &str) -> Option<&ThemeToken> {
        let index = self.inner.tokens.named.get(&namespace)?.get(name)?;
        Some(&self.inner.declarations.tokens[*index])
    }

    /// Every active named token in `namespace`, ordered by custom-property name.
    #[must_use]
    pub fn tokens_in(&self, namespace: ThemeNamespace) -> Vec<&ThemeToken> {
        self.inner
            .tokens
            .named
            .get(&namespace)
            .into_iter()
            .flat_map(BTreeMap::values)
            .map(|index| self.inner.declarations.tokens[*index].as_ref())
            .collect()
    }

    /// The namespace's scalar scale token, when one is active.
    #[must_use]
    pub fn scale(&self, namespace: ThemeNamespace) -> Option<&ThemeToken> {
        self.inner
            .tokens
            .scales
            .get(&namespace)
            .map(|index| self.inner.declarations.tokens[*index].as_ref())
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
        let expected = format_args!("--{namespace}-{name}").to_lean_string();
        self.custom_property(&expected)
    }

    /// Returns the active token that the candidate key `key` selects in the namespace
    /// `namespace`, as Tailwind resolves `--value(--namespace-*)`.
    ///
    /// `namespace` is any namespace name without the leading `--` and the trailing `-`,
    /// such as `size` or `size-lg`. The lookup does not use the classification of
    /// [`ThemeNamespace`]. It also accepts `_` in a token name for `.` in `key`, and it
    /// skips a token of a narrower namespace that Tailwind excludes, such as
    /// `--font-weight-bold` for `font`.
    #[must_use]
    pub fn namespace_token(&self, namespace: &str, key: &str) -> Option<&ThemeToken> {
        let token = namespace_key_names(namespace, key)
            .iter()
            .find_map(|name| self.custom_property(name))?;
        (!excluded_from_namespace(namespace, &token.name.value)).then_some(token)
    }

    /// Returns every active token in the namespace `namespace` with its key, ordered by
    /// custom-property name.
    ///
    /// `namespace` is written as in [`Self::namespace_token`], and the same narrower
    /// namespaces are skipped. A key is the token name after `--{namespace}-`, as
    /// authored, so it keeps CSS escapes.
    #[must_use]
    pub fn namespace_tokens(&self, namespace: &str) -> Vec<(&str, &ThemeToken)> {
        let prefix = format_args!("--{namespace}-").to_lean_string();
        self.active_token_definitions()
            .filter(|token| !excluded_from_namespace(namespace, &token.name.value))
            .filter_map(|token| {
                let key = token.name.value.strip_prefix(prefix.as_str())?;
                (!key.is_empty()).then_some((key, token.as_ref()))
            })
            .collect()
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
        self.inner
            .tokens
            .active
            .get(name)
            .map(|index| self.inner.declarations.tokens[*index].as_ref())
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
    ) -> Result<Option<LeanString>, ThemeReferenceError> {
        resolve_theme_references(value, |name| self.custom_property(name).is_some())
    }

    /// Replaces build-time `theme()` calls in an arbitrary value with the authored
    /// values of active tokens or with the calls' fallbacks.
    ///
    /// Returns [`None`] when a call is malformed or names an inactive token without a
    /// fallback.
    #[must_use]
    pub fn inline_theme_functions(&self, value: &str) -> Option<LeanString> {
        inline_theme_functions(value, |name| {
            self.custom_property(name)
                .map(|token| token.value.value.as_str())
        })
    }

    /// Returns the effective CSS custom properties after v4 namespace resets.
    ///
    /// Resets and replacements are applied in authored order. The result is sorted
    /// by custom-property name because the effective values are stored in a `BTreeMap`.
    #[must_use]
    pub fn active_tokens(&self) -> Vec<&ThemeToken> {
        self.active_token_definitions().map(Arc::as_ref).collect()
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
        omit_rules: &[(LeanStr, LeanStr)],
    ) -> Vec<&ThemeToken> {
        let tokens = self.active_tokens();
        let mut required = custom_property_references(css);
        if include_preflight {
            required.extend(custom_property_references(self.custom_css()));
        }
        for token in &tokens {
            if matches!(token.mode, ThemeTokenMode::Static) {
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
            if css.contains(condition.as_str()) {
                required.retain(|name| !name.starts_with(prefix.as_str()));
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

    /// Authored CSS outside compile-time definition blocks, retained verbatim.
    #[must_use]
    pub fn custom_css(&self) -> &str {
        &self.inner.declarations.custom_css
    }

    /// The last authored user utility with this exact name.
    ///
    /// Wildcard definitions are not matched here; candidate resolution owns
    /// prefix matching.
    #[must_use]
    pub fn utility(&self, name: &str) -> Option<&CustomUtility> {
        self.utilities()
            .iter()
            .rev()
            .find(|utility| utility.name == name)
            .map(Arc::as_ref)
    }

    /// The last authored user variant with this exact name.
    #[must_use]
    pub fn variant(&self, name: &str) -> Option<&CustomVariant> {
        self.variants()
            .iter()
            .rev()
            .find(|variant| variant.name == name)
            .map(Arc::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;
    use crate::parse_theme;
    use crate::theme::declarations::extract_theme_declarations;

    #[test]
    fn clones_share_declarations_and_preparation_without_affecting_new_themes() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --color-brand:red; } @utility brand { color:red; }",
        )
        .unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("theme.css")).unwrap();
        let clone = theme.clone();
        assert!(Arc::ptr_eq(&theme.utilities()[0], &clone.utilities()[0]));
        assert!(std::ptr::eq(theme.programs(), clone.programs()));
        let replacement =
            SourceFile::new(FileId(2), None, None, "@theme { --color-brand:blue; }").unwrap();
        let replacement = parse_theme(&replacement, &CssSourceName::from("theme.css")).unwrap();
        assert_eq!(replacement.value("color", "brand"), Some("blue"));
        assert_eq!(clone.value("color", "brand"), Some("red"));
    }

    #[test]
    fn resets_and_redefinitions_select_only_effective_tokens() {
        let theme = ThemeBuilder {
            tokens: extract_theme_declarations(
                FileId(1),
                "--color-old:red;--color-*:initial;--color-new:blue;--color-new:green;",
                0,
                ThemeTokenMode::Default,
            )
            .unwrap(),
            ..ThemeBuilder::default()
        }
        .finish();
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
        let ordered = ThemeBuilder {
            tokens: extract_theme_declarations(
                FileId(1),
                "--color-z:red;--color-a:blue;--color-*:initial;--color-z:green;--color-a:black;",
                0,
                ThemeTokenMode::Default,
            )
            .unwrap(),
            ..ThemeBuilder::default()
        }
        .finish();
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
        let theme = parse_theme(
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
        let theme = ThemeBuilder {
            tokens: extract_theme_declarations(
                FileId(1),
                "--color-a:var(--color-b);--color-b:var(--color-a);--color-unused:red;",
                0,
                ThemeTokenMode::Default,
            )
            .unwrap(),
            ..ThemeBuilder::default()
        }
        .finish();
        let names = theme
            .tokens_required_by("color:var(--color-a)", false, &[])
            .iter()
            .map(|t| t.name.value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["--color-a", "--color-b"]);
    }
}
