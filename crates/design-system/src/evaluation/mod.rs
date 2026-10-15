//! Candidate evaluation: turning authored spellings into declarations and selectors
//! using only the definitions a snapshot holds.
//!
//! Entry points borrow the immutable [`crate::ThemeIndex`]. The theme owns effective
//! token lookup and parsed utility programs, which its clones and snapshots share.
//!
//! The modules below split the work by subject: [`builtins`] reads the selected
//! registry, [`utilities`] matches and expands a utility body, [`values`] resolves
//! typed value domains and placeholders, and [`variants`] produces selector
//! applications and their ordering. [`context`] records direct selections for one
//! evaluation. [`programs`] prepares utility syntax and declaration branches.
//! [`syntax`] matches delimiters while reading definition expressions.

mod builtins;
mod context;
mod programs;
mod syntax;
mod utilities;
mod values;
mod variants;

pub use builtins::{
    builtin_declaration_formats, builtin_definitions, builtin_omit_token_prefixes,
    builtin_property_definitions, builtin_theme_order, builtin_utility_token_prefixes,
};
pub use utilities::{
    NestedUtilityAtRule, NestedUtilityRule, ResolvedUtility, ResolvedUtilityTarget,
    UtilityArbitraryUse, UtilityOrder, UtilityPropertySort, UtilityResolutionError,
    UtilityTokenUse, resolve_animation_option, resolve_utility,
};
pub use variants::{
    ResolvedVariant, VariantApplication, VariantDeclaration, VariantMediaValue,
    VariantSequenceOrder, VariantValueSource, compare_variant_sequences,
    compatibility_marker_class, document_scoped_selector, is_condition_variant,
    is_tailwind_relationship_candidate, repeated_subject_selector, resolve_variant,
    resolve_variant_with_usage,
};

pub(crate) use programs::UtilityPrograms;

use context::{EvaluationContext, ValueSelections};

#[cfg(test)]
mod tests;
