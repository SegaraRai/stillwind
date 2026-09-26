//! Candidate evaluation: turning authored spellings into declarations and selectors
//! using only the definitions a snapshot holds.
//!
//! Evaluation is pure. Every entry point takes a [`crate::ThemeIndex`] and returns
//! values; nothing is cached between calls, so two snapshots built from different CSS
//! cannot influence one another, and the same input always produces the same output for
//! one snapshot.
//!
//! The modules below split the work by subject: [`builtins`] reads the selected
//! registry, [`utilities`] matches and expands a utility body, [`values`] resolves
//! typed value domains and placeholders, and [`variants`] produces selector
//! applications and their ordering.

mod builtins;
mod syntax;
mod utilities;
mod values;
mod variants;

pub use builtins::{
    builtin_declaration_formats, builtin_definitions, builtin_omit_token_prefixes,
    builtin_property_definitions, builtin_theme_order, builtin_utility_names,
    builtin_utility_token_prefixes, builtin_variant_names,
};
pub use utilities::{
    NestedUtilityAtRule, NestedUtilityRule, ResolvedUtility, ResolvedUtilityTarget, UtilityOrder,
    UtilityPropertySort, UtilityResolutionError, resolve_animation_option, resolve_utility,
};
pub use variants::{
    VariantApplication, VariantDeclaration, compare_variant_sequences, compatibility_marker_class,
    document_scoped_selector, is_condition_variant, is_tailwind_relationship_candidate,
    resolve_variant,
};

#[cfg(test)]
mod tests;
