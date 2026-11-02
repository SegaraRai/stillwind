//! The compilation pipeline.
//!
//! [`Compiler`] exposes the stateless compilation entry points. Callers with reusable
//! snapshots and module artifacts use [`ModuleCompiler`](crate::ModuleCompiler) and
//! [`StylesheetRenderer`](crate::StylesheetRenderer) instead; this module supplies
//! their lowering passes.
//!
//! Modules:
//!
//! - [`animation_options`] resolves the option lists of animation uses.
//! - [`api`] implements the stateless entry points.
//! - [`class_units`] allocates class identities and publishes base CSS before
//!   conditional CSS.
//! - [`conditionals`] lowers runtime value tests after rule construction.
//! - [`css_call`] lowers `style.css()` calls into stylesheet strings.
//! - [`definitions`] emits the CSS that each declaration owns.
//! - [`document`] resolves document-level styles, whose variants select the element to
//!   style.
//! - [`dynamic`] finds the candidates that host plugins must define.
//! - [`editor`] captures source occurrences and reuses emission for isolated CSS
//!   previews.
//! - [`emission`] lowers style inputs.
//! - [`expression_render`] serializes lowered class trees in a class-value context.
//! - [`fragment`] folds ordered contributions and materializes framework props.
//! - [`lexer`] tokenizes utilities and reports span failures as diagnostics.
//! - [`literal`] resolves utility text.
//! - [`media`] lowers `style.media()` calls into media query strings.
//! - [`output`] defines compilation results and structured CSS contributions.
//! - [`pipeline`] orders the passes for one module.
//! - [`props`] carries per-call metadata.
//! - [`rule_boundaries`] assigns merge barriers in final rule order.
//! - [`style_variants`] lowers the variant wrappers of source expressions.
//! - [`stylesheet`] assembles stylesheets and checks the stylesheets that a host marks
//!   for lint.
//! - [`symbols`] generates deterministic identifiers.
//! - [`utilities`] resolves utilities and renders their rules.
//! - [`variants`] defines the non-compound variant steps that authored and generated
//!   CSS renderers share.

mod animation_options;
mod api;
mod class_units;
mod conditionals;
mod css_call;
mod definitions;
mod document;
mod dynamic;
mod editor;
mod emission;
mod expression_render;
mod fragment;
mod lexer;
mod literal;
mod media;
mod output;
mod pipeline;
mod props;
mod rule_boundaries;
mod style_variants;
mod stylesheet;
mod symbols;
mod utilities;
mod variants;

pub use api::Compiler;
pub use editor::{EditorSite, EditorSiteKind, ModuleEditor};
pub use output::{CompileResult, CssContribution};
pub use props::{GeneratedClass, GuardStep};
pub use stylesheet::{StylesheetLint, lint_stylesheets};
pub use symbols::{GeneratedSymbol, GeneratedSymbolKind, SymbolOrigin};

pub(crate) use animation_options::resolve_options;
pub(crate) use conditionals::lower as lower_conditionals;
pub(crate) use definitions::{emit_view_transition_utilities, resolve_frame};
pub(crate) use document::emit_document_style;
pub(crate) use dynamic::{dynamic_candidates, stylesheet_dynamic_candidates};
pub(crate) use editor::{preview_resource_urls, reserve_preview_names};
pub(crate) use media::media_argument_query;
pub(crate) use rule_boundaries::RuleBoundaries;
pub(crate) use stylesheet::{
    apply_candidates, expand_authored_variant, parse_variant_sequence, render_mapped,
    render_module_stylesheet, render_stylesheet, rewrite_generated_css, variant_sequences,
};
pub(crate) use symbols::detached_symbol;
pub(crate) use utilities::{
    FLATTEN_COLOR_COMPATIBILITY, candidate_allowed, check_candidate_name, emit_utility_rules,
    resolve_utility_targets, with_type_hint,
};
pub(crate) use variants::{condition_paths, wrap_conditions};

#[cfg(test)]
mod tests;
