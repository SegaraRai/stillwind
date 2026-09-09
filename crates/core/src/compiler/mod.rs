mod api;
mod authored_directives;
mod css_blocks;
mod declaration_order;
mod handles;
mod keyframes;
mod motion;
mod options;
mod output;
mod pipeline;
mod properties;
mod react_props;
mod react_types;
mod stylesheet;
mod symbols;
mod utilities;
mod utility_emission;
mod utility_render;

pub struct Compiler;

pub use api::CompilerThemeError;
pub use options::{CompilerOptions, CompilerOptionsError};
pub use output::CompileResult;
pub use react_props::GeneratedClass;
pub use symbols::GeneratedSymbol;

pub(crate) use react_types::class_name;
pub(crate) use react_types::render_react_types;
pub(crate) use stylesheet::render_stylesheet;
pub(crate) use utilities::{resolve_utility_targets, unresolved_utility_rule};
pub(crate) use utility_emission::emit_utility_rules;

#[cfg(test)]
mod tests;
