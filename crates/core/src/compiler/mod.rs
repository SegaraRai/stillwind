use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use serde::{Deserialize, Serialize};
use stillwind_grammar::{Variant, lex_utilities, parse_candidate};
use stillwind_ir::{
    AnimationDefinition, AssetReference, AttributeEntry, HandleProjection, StyleInput,
    ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, Candidates, ExportedValue, IdentityId, TypeFact,
};
use stillwind_source::{
    CompileError, LintFinding, SourceFile, Span, Spanned, filter_lint_findings,
    normalized_module_path,
};

mod types;
pub use types::*;
#[derive(Clone, Debug)]
struct ViewTransitionStyleEntry {
    value: Option<Span>,
    identity: Option<String>,
    candidates: Vec<String>,
    key: Option<Span>,
}

#[derive(Clone, Debug)]
struct DynamicAssetClassEntry {
    value: Span,
    classes: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
struct ReactPropsParts<'a> {
    classes: &'a [GeneratedClass],
    attributes: &'a [AttributeEntry],
    handle_markers: &'a [String],
    external_classes: &'a [Span],
    dynamic_handle_classes: &'a [Span],
    dynamic_asset_classes: &'a [DynamicAssetClassEntry],
    view_transition_style: Option<&'a ViewTransitionStyleEntry>,
}

pub struct Compiler;

use crate::{
    DesignSystemSnapshot, Fingerprint, ModuleAnalysis, ModuleCompiler, OxcSourceAnalyzer,
    SourceAnalyzer,
};
mod symbols;
use symbols::{
    asset_identity, asset_symbol_name, generated_class_symbol, generated_ref_marker_symbol,
    generated_ref_symbol, generated_scope_marker_symbol, generated_scope_symbol, generated_symbol,
    handle_projection_name, ref_symbol_name, runtime_projection_class, symbol_name,
};
mod motion;
use motion::{emit_animation_definition, emit_view_transition_definition, resolve_animation_use};
mod handles;
use handles::{
    handle_marker_class, handle_parent_selector, named_identity, ref_identity,
    render_handle_projection, scope_identity, scope_marker_class, summary_runtime_assets,
};
mod react_props;
use react_props::render_react_props;
mod utilities;
use utilities::{CompiledUtility, resolve_utility, view_transition_utility};
pub(crate) use utilities::{resolve_utility_targets, unresolved_utility_rule};
mod utility_emission;
pub(crate) use utility_emission::emit_utility_rules;
mod utility_render;
use utility_render::render_utility_rule;
mod declaration_order;
use declaration_order::{
    combine_declarations_for_theme, order_builtin_utilities, prepend_missing_declarations,
};
mod stylesheet;
pub(crate) use stylesheet::render_stylesheet;
mod authored_directives;
use authored_directives::expand_authored_directives;
mod css_blocks;
use css_blocks::find_at_rule_block;
mod keyframes;
use keyframes::{extract_authored_keyframes, without_keyframes};
mod properties;
use properties::render_used_properties;
mod react_types;
pub(crate) use react_types::class_name;
pub(crate) use react_types::render_react_types;
mod api;
mod pipeline;
#[cfg(test)]
mod tests;
