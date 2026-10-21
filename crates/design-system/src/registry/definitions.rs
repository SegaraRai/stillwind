//! The complete built-in definition set parsed from one CSS buffer.
//!
//! A snapshot keeps the selected built-ins as one immutable value so callers never
//! consult a process-global registry: two snapshots built from different CSS stay
//! independent, and no load can leak definitions into another.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::{CustomPropertyDefinition, parse_property_definitions};

use super::contexts::{RegistryContext, RegistryStatements, mask_registry_blocks};
use super::utility_lookup::UtilityLookup;
use crate::{
    BuiltinArbitraryPropertyOrder, CustomUtility, CustomVariant, MarkerClasses, ThemeNamespaces,
    parse_builtin_arbitrary_property_order, parse_declaration_formats, parse_omit_token_prefixes,
    parse_relationship_patterns, parse_theme_order,
};

/// Immutable registries parsed from the CSS supplied by a host.
///
/// Utility, variant, and property definitions retain authored order and repeated names.
/// Candidate matching preserves each layer's precedence.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BuiltinDefinitions {
    source: LeanStr,
    utilities: Vec<Arc<CustomUtility>>,
    variants: Vec<Arc<CustomVariant>>,
    properties: Vec<Arc<CustomPropertyDefinition>>,
    utility_lookup: UtilityLookup,
    arbitrary_property_order: Vec<BuiltinArbitraryPropertyOrder>,
    declaration_formats: BTreeMap<LeanStr, LeanStr>,
    relationships: BTreeSet<LeanStr>,
    markers: MarkerClasses,
    omit_token_prefixes: Vec<(LeanStr, LeanStr)>,
    theme_order: Vec<LeanStr>,
    theme_namespaces: ThemeNamespaces,
}

impl BuiltinDefinitions {
    /// The exact CSS this value was parsed from. It participates in snapshot
    /// fingerprints, so an upstream definition change invalidates artifacts even when
    /// the parsed structures happen to compare equal.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// All `@stillwind-utility` definitions in authored order.
    pub fn utilities(&self) -> &[Arc<CustomUtility>] {
        &self.utilities
    }

    /// All `@stillwind-variant` definitions in authored order.
    pub fn variants(&self) -> &[Arc<CustomVariant>] {
        &self.variants
    }

    /// All `@stillwind-property` definitions in authored order.
    pub fn properties(&self) -> &[Arc<CustomPropertyDefinition>] {
        &self.properties
    }

    /// Source order for arbitrary properties, used when a utility body sets several of
    /// them and declaration order is observable.
    pub fn arbitrary_property_order(&self) -> &[BuiltinArbitraryPropertyOrder] {
        &self.arbitrary_property_order
    }

    /// Declaration format name to its serialized value grammar.
    pub fn declaration_formats(&self) -> &BTreeMap<LeanStr, LeanStr> {
        &self.declaration_formats
    }

    /// Candidate patterns identifying compatibility relationships.
    pub fn relationships(&self) -> &BTreeSet<LeanStr> {
        &self.relationships
    }

    /// Marker classes that the CSS declares, including `@stillwind-marker-source`
    /// statements, and that built-in definitions select.
    pub fn markers(&self) -> &MarkerClasses {
        &self.markers
    }

    /// Namespace to the prefix a token name omits when it is referenced.
    pub fn omit_token_prefixes(&self) -> &[(LeanStr, LeanStr)] {
        &self.omit_token_prefixes
    }

    /// Theme namespace names in the order utilities consume them.
    pub fn theme_order(&self) -> &[LeanStr] {
        &self.theme_order
    }

    /// Theme namespaces that the CSS declares.
    pub fn theme_namespaces(&self) -> &ThemeNamespaces {
        &self.theme_namespaces
    }

    /// Returns the built-in definitions whose names match the utility name `name`: the
    /// wildcard definitions whose prefix `name` starts with, newest first, followed by
    /// the exact definitions of `name`, newest first.
    pub(crate) fn matching_utilities(&self, name: &str) -> Vec<&CustomUtility> {
        self.utility_lookup.matching(name, &self.utilities, true)
    }

    /// Whether a built-in definition has exactly this name.
    pub(crate) fn has_exact_utility(&self, name: &str) -> bool {
        self.utility_lookup.has_exact(name)
    }

    /// Returns a copy that uses `utilities` and `variants` in place of the built-in
    /// utility and variant definitions and keeps the other registries, including the
    /// marker classes.
    pub(crate) fn with_definitions(
        &self,
        utilities: Vec<Arc<CustomUtility>>,
        variants: Vec<Arc<CustomVariant>>,
    ) -> Self {
        Self {
            utility_lookup: UtilityLookup::new(&utilities),
            utilities,
            variants,
            ..self.clone()
        }
    }

    /// Parses every registry that can be expressed as CSS from `css`.
    ///
    /// Utility bodies retain their CSS token spelling, including meaningful whitespace.
    ///
    /// The utility and variant registries hold the definitions of the `default`
    /// registry context, after its undefinitions apply. Statements that only other
    /// contexts read are not part of them, and a malformed `@stillwind-registry` block
    /// leaves out every built-in utility and variant.
    pub fn from_css(css: &str) -> Self {
        let masked = mask_registry_blocks(css);
        let definitions = RegistryStatements::parse(css, |_| false)
            .map(|statements| statements.definitions(RegistryContext::Default))
            .unwrap_or_default();
        Self::from_parts(
            css,
            masked.as_deref().unwrap_or(css),
            definitions.builtin_utilities,
            definitions.builtin_variants,
        )
    }

    /// Builds the registries with the built-in definitions `utilities` and `variants`.
    /// The other registries are parsed from `css`, a copy of `source` in which every
    /// `@stillwind-registry` block is blank, and `source` is retained as the
    /// fingerprint text.
    pub(crate) fn from_parts(
        source: &str,
        css: &str,
        utilities: Vec<Arc<CustomUtility>>,
        variants: Vec<Arc<CustomVariant>>,
    ) -> Self {
        let utility_lookup = UtilityLookup::new(&utilities);
        let markers = MarkerClasses::collect(
            css,
            utilities.iter().map(|utility| &**utility),
            variants.iter().map(|variant| &**variant),
        );
        Self {
            source: source.into(),
            utilities,
            utility_lookup,
            variants,
            properties: parse_property_definitions(css)
                .into_iter()
                .map(Arc::new)
                .collect(),
            arbitrary_property_order: parse_builtin_arbitrary_property_order(css),
            declaration_formats: parse_declaration_formats(css),
            relationships: parse_relationship_patterns(css),
            markers,
            omit_token_prefixes: parse_omit_token_prefixes(css),
            theme_order: parse_theme_order(css),
            theme_namespaces: ThemeNamespaces::parse(css),
        }
    }
}
