//! The compilation pipeline, split by concern.
//!
//! [`api`] holds the stateless entry points, [`pipeline`] orders the passes, and
//! [`emission`], [`props`], [`style_variants`], and [`variants`] lower and render style calls.
//! [`symbols`], [`handles`], [`motion`], and [`lexer`] allocate names and emit asset definitions,
//! while [`utilities`] and [`stylesheet`] render CSS and [`react`] renders framework type
//! declarations. Only [`api`] re-exports a public surface; everything else is crate-internal
//! and free to change.

mod api;
mod emission;
mod handles;
mod lexer;
mod motion;

mod output;
mod pipeline;
mod props;
mod react;
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

pub use api::CompilerThemeError;
pub use output::CompileResult;
pub use props::{GeneratedClass, GuardStep};
pub use stylesheet::render::StylesheetError;
pub use symbols::GeneratedSymbol;

pub(crate) use react::declarations::render_react_types;
pub(crate) use stylesheet::render::{render_module_stylesheet, render_stylesheet};
pub(crate) use stylesheet::template::render_mapped;
pub(crate) use symbols::class_name;
pub(crate) use utilities::emission::emit_utility_rules;
pub(crate) use utilities::resolve::{resolve_utility_targets, unresolved_utility_rule};

#[cfg(test)]
mod tests;
