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
//!
//! [`Compiler`] exposes the stateless compilation entry points implemented by [`api`].
//! Callers with reusable snapshots and module artifacts use the crate's
//! `ModuleCompiler` and `StylesheetRenderer` interfaces; this module supplies their
//! lowering passes.

mod animation_options;
mod api;
mod class_units;
mod conditionals;
mod definitions;
mod editor;
mod emission;
mod expression_render;
mod fragment;
mod lexer;
mod literal;
mod rule_boundaries;

mod output;
mod pipeline;
mod props;
mod style_variants;
mod stylesheet;
mod symbols;
mod utilities;
mod variants;

pub use api::Compiler;
pub use editor::{EditorSite, EditorSiteKind, ModuleEditor};
pub use output::{CompileResult, CssContribution};
pub use props::{GeneratedClass, GuardStep};
pub use symbols::{GeneratedSymbol, GeneratedSymbolKind, SymbolOrigin};

pub(crate) use animation_options::{OptionVariants, resolve_options};
pub(crate) use conditionals::lower as lower_conditionals;
pub(crate) use definitions::{emit_view_transition_utilities, resolve_frame};
pub(crate) use editor::{preview_resource_urls, reserve_preview_names};
pub(crate) use rule_boundaries::RuleBoundaries;
pub(crate) use stylesheet::{render_mapped, render_module_stylesheet, render_stylesheet};
pub(crate) use symbols::detached_symbol;
pub(crate) use utilities::{
    candidate_allowed, check_candidate_name, emit_utility_rules, resolve_utility_targets,
    unresolved_utility_kind,
};
pub(crate) use variants::{condition_paths, wrap_conditions};

#[cfg(test)]
mod tests;
