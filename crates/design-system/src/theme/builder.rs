//! Collects authored declarations until parsing and origin mapping finish.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::{CssOutputTemplate, CustomPropertyDefinition};
use stillwind_source::Span;

use super::declarations::ThemeToken;
use super::index::ThemeIndex;
use super::parser::{DynamicFamily, UtilityImportance};
use crate::registry::{ArbitraryPropertyRule, RegistryStatements};
use crate::skipped::SkippedRange;
use crate::{
    BrowserTargets, CustomUtility, CustomVariant, MarkerClasses, StillwindDefinitions,
    UtilityOrderMode,
};

#[derive(Debug, Default)]
pub(crate) struct ThemeBuilder {
    /// First declaration offsets after imports are expanded, shared by Tailwind-style
    /// and Stillwind variants.
    pub(crate) variant_positions: BTreeMap<LeanStr, usize>,
    /// Authored positions of utility names, shared by Tailwind-style and Stillwind
    /// utilities: the first declaration offset after imports are expanded. The offset
    /// of a name that no plugin defines is increased by `utility_position_base`.
    pub(crate) utility_positions: BTreeMap<LeanStr, usize>,
    /// Added to the authored positions of utilities that no plugin defines, so plugin
    /// utilities precede them. Zero when no plugin defines a utility.
    pub(crate) utility_position_base: usize,
    /// Structured output positions from the authored entry CSS.
    pub(crate) output_template: CssOutputTemplate,
    /// Named `style.css()` output templates; the last definition of each name.
    pub(crate) css_templates: BTreeMap<LeanStr, CssOutputTemplate>,
    /// `@stillwind-dynamic` utility families, one for each prefix.
    pub(crate) dynamic_utilities: Vec<DynamicFamily>,
    /// Importance of class utilities from a Tailwind plugin configuration.
    pub(crate) important: Option<UtilityImportance>,
    /// Stillwind definitions of this CSS; empty when it has none.
    pub(crate) registry: Arc<StillwindDefinitions>,
    /// Every token in authored order, including resets and superseded values.
    pub(crate) tokens: Vec<ThemeToken>,
    /// Authored CSS outside definition blocks, retained verbatim.
    pub(crate) custom_css: LeanStr,
    /// User `@utility` definitions of the `default` registry context in registration
    /// order.
    pub(crate) utilities: Vec<Arc<CustomUtility>>,
    /// Definition and undefinition statements of every registry context.
    pub(crate) registry_statements: RegistryStatements,
    /// Statements that parsing skipped, with ranges in the parsed buffer, ordered by
    /// range.
    pub(crate) skipped: Vec<SkippedRange>,
    /// Arbitrary property rules of the `default` registry context.
    pub(crate) arbitrary_property_rules: Vec<ArbitraryPropertyRule>,
    /// User variant definitions of the `default` registry context. Definitions from
    /// `@plugin` answers precede the stylesheet's, and each group keeps source order.
    pub(crate) variants: Vec<Arc<CustomVariant>>,
    /// Compiler-owned `@stillwind-property` definitions in authored order.
    pub(crate) properties: Vec<CustomPropertyDefinition>,
    /// Marker classes that user definitions select.
    pub(crate) markers: MarkerClasses,
    /// Generic serialization formats attached to CSS declarations.
    pub(crate) declaration_formats: BTreeMap<LeanStr, LeanStr>,
    /// Candidate patterns identifying compatibility relationships, such as `group-*`.
    pub(crate) relationship_patterns: BTreeSet<LeanStr>,
    /// Property names declared by authored `@property` rules.
    pub(crate) authored_properties: BTreeSet<LeanStr>,
    /// `syntax` descriptors of authored `@property` rules, keyed by property name.
    pub(crate) authored_property_syntaxes: BTreeMap<LeanStr, LeanStr>,
    /// Host-selected ordering strategy for resolved utilities.
    pub(crate) utility_order: UtilityOrderMode,
    /// Minimum browser versions used to remove generated color compatibility branches.
    pub(crate) target: BrowserTargets,
}

impl ThemeBuilder {
    /// Rewrites the origins of the entry template and every named template through
    /// `map`, as [`CssOutputTemplate::map_origins`] describes.
    pub(crate) fn map_template_origins(&mut self, mut map: impl FnMut(Span) -> Option<Span>) {
        self.output_template.map_origins(&mut map);
        for template in self.css_templates.values_mut() {
            template.map_origins(&mut map);
        }
    }

    /// Freezes authored declarations and prepares shared lookup and utility syntax.
    /// Invalid utility requirements remain deferred until a matching candidate is used.
    pub(crate) fn finish(self) -> ThemeIndex {
        ThemeIndex::new(self)
    }
}
