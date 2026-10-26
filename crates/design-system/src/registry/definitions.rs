//! The Stillwind definitions parsed from one CSS buffer.
//!
//! A snapshot keeps the Stillwind definitions as one immutable value so callers never
//! consult a process-global registry: two snapshots built from different CSS stay
//! independent, and no load can leak definitions into another.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::{CustomPropertyDefinition, parse_property_definitions};

use super::contexts::{RegistryContext, RegistryStatements, mask_registry_blocks};
use super::formats::{LegacyThemeKey, parse_legacy_theme_keys, parse_legacy_utilities};
use super::utility_lookup::UtilityLookup;
use crate::{
    ArbitraryPropertyOrder, CustomUtility, CustomVariant, MarkerClasses, ThemeNamespaces,
    parse_arbitrary_property_order, parse_declaration_formats, parse_omit_token_prefixes,
    parse_relationship_patterns, parse_theme_order,
};

/// Immutable registries parsed from the CSS supplied by a host.
///
/// Utility, variant, and property definitions retain authored order and repeated names.
/// Candidate matching preserves each layer's precedence.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StillwindDefinitions {
    source: LeanStr,
    utilities: Vec<Arc<CustomUtility>>,
    variants: Vec<Arc<CustomVariant>>,
    properties: Vec<Arc<CustomPropertyDefinition>>,
    utility_lookup: UtilityLookup,
    arbitrary_property_order: Vec<ArbitraryPropertyOrder>,
    declaration_formats: BTreeMap<LeanStr, LeanStr>,
    relationships: BTreeSet<LeanStr>,
    markers: MarkerClasses,
    omit_token_prefixes: Vec<(LeanStr, LeanStr)>,
    theme_order: Vec<LeanStr>,
    theme_namespaces: ThemeNamespaces,
    legacy_utilities: Vec<(LeanStr, LeanStr)>,
    legacy_theme_keys: BTreeMap<LeanStr, LegacyThemeKey>,
}

impl StillwindDefinitions {
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
    pub fn arbitrary_property_order(&self) -> &[ArbitraryPropertyOrder] {
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
    /// statements, and that the Stillwind definitions select.
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

    /// Pairs of a legacy utility name and its current name, from
    /// `@stillwind-legacy-utility` statements in authored order. A trailing `*` in both
    /// names keeps the rest of the name.
    pub fn legacy_utilities(&self) -> &[(LeanStr, LeanStr)] {
        &self.legacy_utilities
    }

    /// Maps each Tailwind v3 config key to the theme namespace that it reads, such as
    /// `text` for `fontSize`, from `@stillwind-legacy-theme-key` statements.
    pub fn legacy_theme_keys(&self) -> &BTreeMap<LeanStr, LegacyThemeKey> {
        &self.legacy_theme_keys
    }

    /// Returns the `@stillwind-utility` definitions whose names match the utility name
    /// `name`: the wildcard definitions whose prefix `name` starts with, newest first,
    /// followed by the exact definitions of `name`, newest first.
    pub(crate) fn matching_utilities(&self, name: &str) -> Vec<&CustomUtility> {
        self.utility_lookup.matching(name, &self.utilities, true)
    }

    /// Whether a `@stillwind-utility` definition has exactly this name.
    pub(crate) fn has_exact_utility(&self, name: &str) -> bool {
        self.utility_lookup.has_exact(name)
    }

    /// Returns a copy that uses `utilities` and `variants` in place of the
    /// `@stillwind-utility` and `@stillwind-variant` definitions and keeps the other
    /// registries, including the marker classes.
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
    /// contexts read are not part of them.
    pub fn from_css(css: &str) -> Self {
        let masked = mask_registry_blocks(css);
        let definitions = RegistryStatements::parse(css, |_| false, &mut Vec::new())
            .definitions(RegistryContext::Default);
        Self::from_parts(
            css,
            masked.as_deref().unwrap_or(css),
            definitions.stillwind_utilities,
            definitions.stillwind_variants,
        )
    }

    /// Builds the registries with the Stillwind definitions `utilities` and `variants`.
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
            arbitrary_property_order: parse_arbitrary_property_order(css),
            declaration_formats: parse_declaration_formats(css),
            relationships: parse_relationship_patterns(css),
            markers,
            omit_token_prefixes: parse_omit_token_prefixes(css),
            theme_order: parse_theme_order(css),
            theme_namespaces: ThemeNamespaces::parse(css),
            legacy_utilities: parse_legacy_utilities(css),
            legacy_theme_keys: parse_legacy_theme_keys(css),
        }
    }
}
