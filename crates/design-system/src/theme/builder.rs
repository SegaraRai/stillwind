//! Collects authored declarations until parsing and origin mapping finish.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::{CssOutputTemplate, CustomPropertyDefinition};
use stillwind_source::Span;

use super::declarations::ThemeToken;
use super::index::ThemeIndex;
use super::parser::{DynamicFamily, UtilityImportance};
use crate::{BrowserTargets, BuiltinDefinitions, CustomUtility, CustomVariant, UtilityOrderMode};

#[derive(Debug, Default)]
pub(crate) struct ThemeBuilder {
    /// First declaration offsets after imports are expanded, shared by user and
    /// built-in variants.
    pub(crate) variant_positions: BTreeMap<LeanStr, usize>,
    /// Authored positions of utility names, shared by user and built-in utilities: the
    /// first declaration offset after imports are expanded. The offset of a name that
    /// no plugin defines is increased by `utility_position_base`.
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
    /// Built-in definitions selected by this CSS; empty when it imports none.
    pub(crate) registry: Arc<BuiltinDefinitions>,
    /// Every token in authored order, including resets and superseded values.
    pub(crate) tokens: Vec<ThemeToken>,
    /// Authored CSS outside definition blocks, retained verbatim.
    pub(crate) custom_css: LeanStr,
    /// User `@utility` definitions in authored order.
    pub(crate) utilities: Vec<CustomUtility>,
    /// User variant definitions grouped by syntax form; source order within each form.
    pub(crate) variants: Vec<CustomVariant>,
    /// Compiler-owned `@stillwind-property` definitions in authored order.
    pub(crate) properties: Vec<CustomPropertyDefinition>,
    /// Utility names whose declarations provide compatibility marker classes.
    pub(crate) marker_utilities: BTreeSet<LeanStr>,
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
