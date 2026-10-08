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
//! [`output`] defines compilation results and structured CSS contributions.
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

/// Marker type for the crate's associated compilation functions.
///
/// [`Compiler`] has no instances and no state; every entry point takes the inputs it needs and
/// returns a result.
pub struct Compiler;

pub use output::CompileResult;
pub use output::CssContribution;
pub use props::{GeneratedClass, GuardStep};
pub use symbols::{GeneratedSymbol, GeneratedSymbolKind, SymbolOrigin};

pub(crate) use rule_boundaries::RuleBoundaries;
pub(crate) use stylesheet::render_mapped;
pub(crate) use stylesheet::{render_module_stylesheet, render_stylesheet};
pub(crate) use symbols::detached_symbol;
pub(crate) use utilities::emit_utility_rules;
pub(crate) use utilities::{candidate_allowed, check_candidate_name};
pub(crate) use utilities::{resolve_utility_targets, unresolved_utility_kind};

#[cfg(test)]
mod tests;
