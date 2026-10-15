//! Collects authored declarations until parsing and origin mapping finish.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::{CssOutputTemplate, CustomPropertyDefinition};

use super::declarations::ThemeToken;
use super::index::ThemeIndex;
use crate::{BrowserTargets, BuiltinDefinitions, CustomUtility, CustomVariant, UtilityOrderMode};

#[derive(Debug, Default)]
pub(crate) struct ThemeBuilder {
    /// First declaration offsets after imports are expanded, shared by user and built-in variants.
    pub(crate) variant_positions: BTreeMap<LeanStr, usize>,
    /// First declaration offsets after imports are expanded, shared by user and built-in utilities.
    pub(crate) utility_positions: BTreeMap<LeanStr, usize>,
    /// Structured output positions from the authored entry CSS.
    pub(crate) output_template: CssOutputTemplate,
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
    /// Host-selected ordering strategy for resolved utilities.
    pub(crate) utility_order: UtilityOrderMode,
    /// Minimum browser versions used to remove generated color compatibility branches.
    pub(crate) target: BrowserTargets,
}

impl ThemeBuilder {
    /// Freezes authored declarations and prepares shared lookup and utility syntax.
    /// Invalid utility requirements remain deferred until a matching candidate is used.
    pub(crate) fn finish(self) -> ThemeIndex {
        ThemeIndex::new(self)
    }
}
