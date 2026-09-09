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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
    pub guard_when_true: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PropsTransform {
    pub call_span: Span,
    pub replacement: String,
}

/// A generated CSS identifier with the canonical identity used to allocate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    pub name: String,
    pub kind: String,
    pub identity: String,
    pub source_path: String,
    pub start: u32,
    pub end: u32,
}

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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CompileResult {
    pub css: String,
    /// Unwrapped utility rules used to assemble one stylesheet for a module graph.
    #[serde(skip)]
    pub module_css: String,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub transforms: Vec<PropsTransform>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub lint_policy: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    pub prefix: String,
    pub tailwind_prefix: String,
    pub forbid_tailwind_relationships: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
            forbid_tailwind_relationships: false,
        }
    }
}

impl CompilerOptions {
    /// Validates that generated and Tailwind-compatible namespaces are distinct CSS prefixes.
    ///
    /// # Errors
    ///
    /// Returns an error when a prefix is malformed or both namespaces are identical.
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("prefix", self.prefix.as_str()),
            ("tailwindPrefix", self.tailwind_prefix.as_str()),
        ] {
            if !value.starts_with("--") || !value.ends_with('-') || value.len() < 4 {
                return Err(format!("{name} must start with `--` and end with `-`"));
            }
        }
        if self.prefix == self.tailwind_prefix {
            return Err("prefix and tailwindPrefix must be different".to_owned());
        }
        Ok(())
    }

    pub(crate) fn rewrite_generated(&self, value: &str) -> String {
        value
            .replace("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
    }
}

impl CompileResult {
    /// Applies source-spanned macro replacements from right to left.
    ///
    /// This deliberately keeps the compiler's source mapping contract separate
    /// from any particular bundler. A Vite adapter can use the same plan and
    /// provide a high-resolution source map around these exact byte spans.
    ///
    /// # Errors
    ///
    /// Returns an error when a transform targets another file or overlaps a
    /// previously applied source span.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, String> {
        let mut transforms = self.transforms.clone();
        transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

        let mut output = source.text().to_owned();
        let mut previous_start = u32::MAX;
        for transform in transforms {
            if source.slice(transform.call_span).is_err()
                || transform.call_span.end.0 > previous_start
                || transform.call_span.end.0 as usize > output.len()
            {
                return Err("invalid or overlapping stillwind transform plan".to_owned());
            }
            let start = transform.call_span.start.0 as usize;
            let end = transform.call_span.end.0 as usize;
            output.replace_range(start..end, &transform.replacement);
            previous_start = transform.call_span.start.0;
        }
        Ok(output)
    }
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
