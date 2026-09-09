//! CSS-first theme parsing. Lightning CSS validates the stylesheet before stillwind
//! extracts its small, source-spanned `@theme` surface.

mod blocks;
mod builtins;
mod catalog;
mod colors;
mod definitions;
mod directives;
mod domains;
mod format_registry;
mod imports;
mod math_expression;
mod math_folding;
mod math_parser;
mod properties;
mod property_usage;
mod registry;
mod theme_declarations;
mod theme_domains;
mod theme_index;
mod theme_parser;
mod theme_references;
mod theme_types;
mod utility_definitions;
mod value_syntax;
mod values;
mod variants;

#[cfg(test)]
mod tests;

pub use builtins::*;
pub use catalog::{TAILWIND_V4_PREFLIGHT, csswg_formal_syntax};
pub use colors::*;
pub use definitions::*;
pub use domains::*;
pub use format_registry::*;
pub use math_folding::*;
pub use properties::*;
pub use property_usage::*;
pub use registry::*;
pub use theme_domains::*;
pub use theme_index::*;
pub use theme_parser::*;
pub use theme_types::*;
pub use values::*;
