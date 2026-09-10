mod api;
mod handles;
mod lexer;
mod motion;
mod options;
mod output;
mod pipeline;
mod react;
mod stylesheet;
mod symbols;
mod utilities;
mod variants;

pub struct Compiler;

pub use api::CompilerThemeError;
pub use options::{ClassIdentity, CompilerOptions, CompilerOptionsError};
pub use output::CompileResult;
pub use react::props::GeneratedClass;
pub use stylesheet::render::StylesheetError;
pub use symbols::GeneratedSymbol;

pub(crate) use react::declarations::render_react_types;
pub(crate) use stylesheet::render::{render_module_stylesheet, render_stylesheet};
pub(crate) use symbols::class_name;
pub(crate) use utilities::emission::emit_utility_rules;
pub(crate) use utilities::resolve::{resolve_utility_targets, unresolved_utility_rule};

#[cfg(test)]
mod tests;
