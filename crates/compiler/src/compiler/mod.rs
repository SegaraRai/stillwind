//! The compilation pipeline, split by concern.
//!
//! [`api`] holds the stateless entry points, [`pipeline`] orders the passes, and
//! [`emission`] lowers style inputs, [`props`] carries call metadata, and
//! [`style_variants`] and [`variants`] resolve variant wrappers. [`animation_options`]
//! resolves option lists for animation uses and group options. [`fragment`] folds
//! ordered contributions and materializes framework props. [`conditionals`] lowers
//! runtime value tests after rule construction. [`rule_boundaries`] assigns merge
//! barriers in final rule order. [`literal`] resolves utility text; [`class_units`]
//! allocates classes and orders CSS. [`expression_render`] serializes lowered class
//! trees in class-value context. [`symbols`], [`definitions`], and [`lexer`] allocate
//! names and emit asset definitions. [`utilities`] and [`stylesheet`] render CSS.
//! [`output`] defines compilation results and structured CSS contributions. [`editor`]
//! captures source occurrences and reuses emission for isolated CSS previews.
//! [`document`] resolves document-level styles, whose variants select the element to
//! style. [`media`] lowers `style.media()` calls into media query strings, and
//! [`css_call`] lowers `style.css()` calls into stylesheet strings. [`dynamic`] finds
//! the candidates that host plugins must define. [`stylesheet`] also checks the
//! stylesheets that a host marks for lint.
//!
//! [`Compiler`] exposes the stateless compilation entry points implemented by [`api`].
//! Callers with reusable snapshots and module artifacts use the crate's
//! `ModuleCompiler` and `StylesheetRenderer` interfaces; this module supplies their
//! lowering passes.

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

pub(crate) use animation_options::{OptionVariants, resolve_options};
pub(crate) use conditionals::lower as lower_conditionals;
pub(crate) use definitions::{emit_view_transition_utilities, resolve_frame};
pub(crate) use document::emit_document_style;
pub(crate) use dynamic::dynamic_candidates;
pub(crate) use editor::{preview_resource_urls, reserve_preview_names};
pub(crate) use media::media_argument_query;
pub(crate) use rule_boundaries::RuleBoundaries;
pub(crate) use stylesheet::{
    expand_authored_variant, parse_variant_sequence, render_mapped, render_module_stylesheet,
    render_stylesheet, variant_sequences,
};
pub(crate) use symbols::detached_symbol;
pub(crate) use utilities::{
    FLATTEN_COLOR_COMPATIBILITY, candidate_allowed, check_candidate_name, emit_utility_rules,
    resolve_utility_targets, with_type_hint,
};
pub(crate) use variants::{condition_paths, wrap_conditions};

#[cfg(test)]
mod tests;
