//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

mod arbitrary;
mod colors;
mod declarations;
mod formatting;
mod metadata;
mod nested;
mod normalization;
mod placeholders;
mod registry;
mod theme_values;
mod utilities;
mod utility_body;
mod utility_matching;
mod value_domains;
mod value_macros;
mod variant_body;
mod variant_order;
mod variant_substitution;
mod variants;

#[cfg(test)]
mod tests;

pub use metadata::*;
pub use nested::{NestedUtilityAtRule, NestedUtilityRule};
pub use registry::*;
pub use utilities::*;
pub use variant_order::*;
pub use variants::*;
