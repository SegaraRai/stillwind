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
use super::namespace::namespace_key_names;
use super::parser::{DynamicFamily, UtilityImportance};
use super::references::{inline_theme_functions, resolve_css_functions, resolve_theme_references};
use super::tokens::TokenLookup;
use crate::evaluation::{UtilityPrograms, color_fallback};
use crate::registry::{
    ArbitraryPropertyRule, RegistryContext, RegistryStatements, UtilityLookup,
    accepts_arbitrary_property,
};
use crate::{
    BrowserTargets, BuiltinDefinitions, BypassedUndefinition, CustomUtility, CustomVariant,
    DynamicFamilyKind, MarkerClasses, ThemeNamespaces, ThemeReferenceError, ThemeTokenMode,
    UtilityOrderMode,
};

/// Everything that one stylesheet declares, as the parser collected it. Equality of two
/// indexes compares these declarations only, because the lookups derive from them.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ThemeDeclarations {
    variant_positions: BTreeMap<LeanStr, usize>,
    utility_positions: BTreeMap<LeanStr, usize>,
    utility_position_base: usize,
    output_template: CssOutputTemplate,
    css_templates: BTreeMap<LeanStr, CssOutputTemplate>,
    dynamic_utilities: Vec<DynamicFamily>,
    important: Option<UtilityImportance>,
    registry: Arc<BuiltinDefinitions>,
    tokens: Vec<Arc<ThemeToken>>,
    custom_css: LeanStr,
    utilities: Vec<Arc<CustomUtility>>,
    registry_statements: Arc<RegistryStatements>,
    arbitrary_property_rules: Vec<ArbitraryPropertyRule>,
    variants: Vec<Arc<CustomVariant>>,
    properties: Vec<Arc<CustomPropertyDefinition>>,
    markers: MarkerClasses,
    declaration_formats: BTreeMap<LeanStr, LeanStr>,
    relationship_patterns: BTreeSet<LeanStr>,
    authored_properties: BTreeSet<LeanStr>,
    authored_property_syntaxes: BTreeMap<LeanStr, LeanStr>,
    utility_order: UtilityOrderMode,
    target: BrowserTargets,
}

/// Declarations and the lookups derived from them, shared by every clone of an index.
#[derive(Debug)]
struct ThemeData {
    declarations: ThemeDeclarations,
    /// Active tokens after resets, replacements, and precedence. Context indexes share
    /// the lookup of the index they derive from.
    tokens: Arc<TokenLookup>,
    /// Name lookup over `declarations.utilities`.
    utilities: UtilityLookup,
    /// Prepared utility bodies, built once when the index is created.
    programs: OnceLock<UtilityPrograms>,
    /// The context whose definitions this index holds, or `None` for the default
    /// definitions.
    context: Option<RegistryContext>,
    /// Indexes for each context, in the order of [`RegistryContext::ALL`], built on
    /// first use.
    contexts: [OnceLock<ThemeIndex>; RegistryContext::ALL.len()],
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
    /// Builds an index from parsed declarations, derives the token and utility lookups,
    /// and prepares the utility programs.
    pub(crate) fn new(builder: ThemeBuilder) -> Self {
        let declarations = ThemeDeclarations {
            variant_positions: builder.variant_positions,
            utility_positions: builder.utility_positions,
            utility_position_base: builder.utility_position_base,
            output_template: builder.output_template,
            css_templates: builder.css_templates,
            dynamic_utilities: builder.dynamic_utilities,
            important: builder.important,
            registry: builder.registry,
            tokens: builder.tokens.into_iter().map(Arc::new).collect(),
            custom_css: builder.custom_css,
            utilities: builder.utilities,
            registry_statements: Arc::new(builder.registry_statements),
            arbitrary_property_rules: builder.arbitrary_property_rules,
            variants: builder.variants,
            properties: builder.properties.into_iter().map(Arc::new).collect(),
            markers: builder.markers,
            declaration_formats: builder.declaration_formats,
            relationship_patterns: builder.relationship_patterns,
            authored_properties: builder.authored_properties,
            authored_property_syntaxes: builder.authored_property_syntaxes,
            utility_order: builder.utility_order,
            target: builder.target,
        };
        let tokens = Arc::new(TokenLookup::new(
            &declarations.tokens,
            declarations.registry.theme_namespaces(),
        ));
        Self::from_declarations(declarations, tokens, None, None)
    }

    /// Builds an index over `declarations` and prepares its utility bodies. Bodies that
    /// `base` already prepared are shared instead of parsed again.
    fn from_declarations(
        declarations: ThemeDeclarations,
        tokens: Arc<TokenLookup>,
        context: Option<RegistryContext>,
        base: Option<&UtilityPrograms>,
    ) -> Self {
        let utilities = UtilityLookup::new(&declarations.utilities);
        let theme = Self {
            inner: Arc::new(ThemeData {
                declarations,
                tokens,
                utilities,
                programs: OnceLock::new(),
                context,
                contexts: Default::default(),
            }),
        };
        let _ = theme.inner.programs.set(UtilityPrograms::new(&theme, base));
        theme
    }

    /// Returns the index whose utility, variant, and arbitrary-property definitions are
    /// those of the registry `context`.
    ///
    /// The index is built on first use and shared by every clone of this index. It
    /// shares tokens, templates, and the other declarations with this index. Callers
    /// pass the default index, which [`RegistryContext::Default`] returns; an index of
    /// `context` returns itself.
    #[must_use]
    pub fn context(&self, context: RegistryContext) -> &Self {
        if context == RegistryContext::Default || self.inner.context == Some(context) {
            return self;
        }
        self.inner.contexts[context.index()].get_or_init(|| self.derive(context))
    }

    /// The context whose definitions this index holds, or `None` for the default
    /// definitions.
    #[must_use]
    pub fn registry_context(&self) -> Option<RegistryContext> {
        self.inner.context
    }

    /// Builds the index of `context` from the registry statements of this index.
    fn derive(&self, context: RegistryContext) -> Self {
        let declarations = &self.inner.declarations;
        let definitions = declarations.registry_statements.definitions(context);
        let derived = ThemeDeclarations {
            registry: Arc::new(
                declarations
                    .registry
                    .with_definitions(definitions.builtin_utilities, definitions.builtin_variants),
            ),
            utilities: definitions.utilities,
            variants: definitions.variants,
            arbitrary_property_rules: definitions.arbitrary_properties,
            ..declarations.clone()
        };
        Self::from_declarations(
            derived,
            Arc::clone(&self.inner.tokens),
            Some(context),
            Some(self.programs()),
        )
    }

    /// Whether a `@stillwind-undef-utility` statement of the `default` registry context
    /// matches the utility name `name`.
    #[must_use]
    pub fn undefines_utility(&self, name: &str) -> bool {
        self.inner
            .declarations
            .registry_statements
            .undefines_utility(RegistryContext::Default, name)
    }

    /// Returns the undefinition of the `keyframe` or `view-transition` registry that a
    /// stylesheet definition outside every `@stillwind-registry` block bypasses, as
    /// [`BypassedUndefinition`] describes. The definition defines a utility when
    /// `utility` is true and a variant otherwise, has the name `name`, and starts at
    /// the byte offset `offset` of the parsed text. It must not be inside an `@plugin`
    /// answer.
    #[must_use]
    pub fn bypassed_undefinition(
        &self,
        utility: bool,
        name: &str,
        offset: usize,
    ) -> Option<BypassedUndefinition> {
        self.inner
            .declarations
            .registry_statements
            .bypassed_undefinition(utility, name, offset)
    }

    /// Whether arbitrary-property candidates such as `[color:red]` may set `property`
    /// with these definitions.
    pub(crate) fn accepts_arbitrary_property(&self, property: &str) -> bool {
        accepts_arbitrary_property(&self.inner.declarations.arbitrary_property_rules, property)
    }

    /// Returns the prepared bodies of the project and built-in utilities, shared by
    /// every clone of this index.
    pub(crate) fn programs(&self) -> &UtilityPrograms {
        self.inner
            .programs
            .get_or_init(|| UtilityPrograms::new(self, None))
    }

    /// Returns the project definitions whose names match the utility name `name`: the
    /// exact definitions of `name` and the wildcard definitions whose prefix `name`
    /// starts with, newest first. `(functional)` definitions are not included; see
    /// [`Self::functional_utilities`].
    pub(crate) fn matching_utilities(&self, name: &str) -> Vec<&CustomUtility> {
        self.inner.utilities.matching(name, self.utilities(), false)
    }

    /// Returns the project's `(functional)` definitions of one complete candidate, in
    /// source order.
    pub(crate) fn functional_utilities(&self, candidate: &str) -> Vec<&CustomUtility> {
        self.inner.utilities.functional(candidate, self.utilities())
    }

    /// Whether a project definition without the `(functional)` flag has exactly this
    /// name.
    pub(crate) fn has_exact_utility(&self, name: &str) -> bool {
        self.inner.utilities.has_exact(name)
    }

    /// Returns the definitions that are active after resets, replacements, and
    /// precedence, ordered by custom-property name.
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

    /// Authored positions of utility names, shared by user and built-in utilities.
    ///
    /// A position is the first declaration offset after imports are expanded. Utilities
    /// that a `@plugin` statement defines keep their offsets, and the offsets of other
    /// utilities are increased by [`Self::utility_position_base`], so plugin utilities
    /// come first.
    pub fn utility_positions(&self) -> &BTreeMap<LeanStr, usize> {
        &self.inner.declarations.utility_positions
    }

    /// The amount that authored positions of utilities without a plugin definition
    /// are increased by. Zero when no plugin defines a utility. Other authored
    /// positions in the same text, such as those of arbitrary properties, must be
    /// increased by the same amount.
    pub fn utility_position_base(&self) -> usize {
        self.inner.declarations.utility_position_base
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

    /// Families whose candidates a host plugin defines on demand: the native families
    /// of `@stillwind-dynamic` statements, in the order of their first statements,
    /// followed by the families of `@stillwind-plugin-family` statements that the
    /// Tailwind plugin adapter writes. The host adds a definition for each candidate
    /// of such a family that a module uses.
    pub fn dynamic_utilities(&self) -> &[DynamicFamily] {
        &self.inner.declarations.dynamic_utilities
    }

    /// Returns the native dynamic family that a utility name belongs to, and whether
    /// the name is in the bracket form. Tailwind plugin families are not considered,
    /// because their requests keep the complete candidate.
    ///
    /// A name belongs to a family when it is the prefix followed by a nonempty rest in
    /// a form that the family accepts. The bracket form is `<prefix>[<value>]` with a
    /// nonempty value that contains no `:`, `[`, `]`, or space.
    pub fn dynamic_family(&self, name: &str) -> Option<(&DynamicFamily, bool)> {
        self.dynamic_utilities().iter().find_map(|family| {
            if family.kind != DynamicFamilyKind::Native {
                return None;
            }
            let rest = name.strip_prefix(family.prefix.as_str())?;
            if bracket_value(rest).is_some() {
                family.bracket.then_some((family, true))
            } else {
                (family.named && !rest.is_empty()).then_some((family, false))
            }
        })
    }

    /// Returns the named spelling of `name` when it is the bracket form of a candidate
    /// of a family that accepts both forms, such as `icon-mdi--close` for
    /// `icon-[mdi--close]`.
    pub fn dynamic_named_utility(&self, name: &str) -> Option<LeanString> {
        let (family, true) = self.dynamic_family(name)? else {
            return None;
        };
        let value = bracket_value(&name[family.prefix.len()..])?;
        family
            .named
            .then(|| format_args!("{}{value}", family.prefix).to_lean_string())
    }

    /// Built-in definitions selected by this CSS; empty when it imports none.
    pub fn registry(&self) -> &Arc<BuiltinDefinitions> {
        &self.inner.declarations.registry
    }

    /// Every token in authored order, including resets and superseded values.
    pub fn tokens(&self) -> &[Arc<ThemeToken>] {
        &self.inner.declarations.tokens
    }

    /// Project `@utility` definitions in authored order, including the definitions in
    /// plugin CSS and `(functional)` definitions.
    pub fn utilities(&self) -> &[Arc<CustomUtility>] {
        &self.inner.declarations.utilities
    }

    /// User variant definitions. Definitions from `@plugin` answers precede the
    /// stylesheet's, and each group keeps source order.
    pub fn variants(&self) -> &[Arc<CustomVariant>] {
        &self.inner.declarations.variants
    }

    /// Compiler-owned `@stillwind-property` definitions in authored order.
    pub fn properties(&self) -> &[Arc<CustomPropertyDefinition>] {
        &self.inner.declarations.properties
    }

    /// Marker classes that user definitions select. [`BuiltinDefinitions::markers`]
    /// holds the markers that the stylesheet declares and that built-ins select.
    pub fn markers(&self) -> &MarkerClasses {
        &self.inner.declarations.markers
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

    /// Importance of class utilities that a Tailwind plugin configuration selects.
    /// `None` leaves class utilities as their candidates spell them.
    pub fn important(&self) -> Option<&UtilityImportance> {
        self.inner.declarations.important.as_ref()
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

    /// Theme namespaces that the selected built-in definitions declare.
    #[must_use]
    pub fn namespaces(&self) -> &ThemeNamespaces {
        self.registry().theme_namespaces()
    }

    /// The named token with the key `name` in the declared namespace `namespace`, or
    /// `None` when it is not active.
    #[must_use]
    pub fn token_in(&self, namespace: &str, name: &str) -> Option<&ThemeToken> {
        let index = self.inner.tokens.named.get(namespace)?.get(name)?;
        Some(&self.inner.declarations.tokens[*index])
    }

    /// Every active named token in the declared namespace `namespace` with its key,
    /// ordered by key.
    #[must_use]
    pub fn tokens_in(&self, namespace: &str) -> Vec<(&str, &ThemeToken)> {
        self.inner
            .tokens
            .named
            .get(namespace)
            .into_iter()
            .flatten()
            .map(|(key, index)| {
                (
                    key.as_str(),
                    self.inner.declarations.tokens[*index].as_ref(),
                )
            })
            .collect()
    }

    /// The scale token of the declared namespace `namespace`, such as `--spacing`,
    /// when one is active.
    #[must_use]
    pub fn scale(&self, namespace: &str) -> Option<&ThemeToken> {
        self.inner
            .tokens
            .scales
            .get(namespace)
            .map(|index| self.inner.declarations.tokens[*index].as_ref())
    }

    /// Looks up a token by authored namespace text and bare name.
    ///
    /// A namespace that is not a declared theme namespace falls back to treating the
    /// arguments as a custom-property name, so callers may query an arbitrary property
    /// without knowing whether it participates in a namespace.
    #[must_use]
    pub fn token(&self, namespace: &str, name: &str) -> Option<&ThemeToken> {
        if self.namespaces().get(namespace).is_some() {
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
    /// declared namespaces. It also accepts `_` in a token name for `.` in `key`, and
    /// it skips a token of a narrower namespace that [`ThemeNamespaces::excludes`]
    /// excludes, such as `--font-weight-bold` for `font`.
    #[must_use]
    pub fn namespace_token(&self, namespace: &str, key: &str) -> Option<&ThemeToken> {
        let token = namespace_key_names(namespace, key)
            .iter()
            .find_map(|name| self.custom_property(name))?;
        (!self.namespaces().excludes(namespace, &token.name.value)).then_some(token)
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
            .filter(|token| !self.namespaces().excludes(namespace, &token.name.value))
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

    /// Lowers the scale functions of declared namespaces, such as `--spacing(<value>)`,
    /// and Tailwind's public `--alpha(<color> / <alpha>)` function. An inline scale
    /// contributes its value, and any other scale a `var()` reference.
    ///
    /// Returns `None` for a malformed call, for a scale function whose scale token is
    /// not active, and for expansion beyond the size or nesting limits.
    pub fn resolve_css_functions(&self, value: &str) -> Option<LeanString> {
        let scales = self
            .namespaces()
            .scales()
            .map(|namespace| {
                let scale = self.scale(&namespace.name).map(|token| {
                    if matches!(token.mode, ThemeTokenMode::Inline) {
                        token.value.value.to_lean_string()
                    } else {
                        format_args!("var({})", token.name.value).to_lean_string()
                    }
                });
                (namespace.scale_token(), scale)
            })
            .collect::<Vec<_>>();
        resolve_css_functions(value, &scales)
    }

    /// Returns the effective CSS custom properties after v4 namespace resets.
    ///
    /// Resets and replacements are applied in authored order, and a definition never
    /// replaces or resets one of higher [`crate::ThemeTokenPrecedence`]. The result is
    /// sorted by custom-property name because the effective values are stored in a
    /// `BTreeMap`.
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

    /// The last authored project utility with this exact name, including a
    /// `(functional)` definition.
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

/// Returns the value of a bracket form `[<value>]` whose value is nonempty and contains
/// no `:`, `[`, `]`, or space.
fn bracket_value(rest: &str) -> Option<&str> {
    let value = rest.strip_prefix('[')?.strip_suffix(']')?;
    (!value.is_empty() && !value.contains([':', '[', ']', ' '])).then_some(value)
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
