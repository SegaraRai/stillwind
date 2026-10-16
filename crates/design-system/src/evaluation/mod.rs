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
//! [`syntax`] matches delimiters while reading definition expressions. [`suggestions`]
//! proposes spellings for unknown utility and variant names.

mod builtins;
mod context;
mod programs;
mod suggestions;
mod syntax;
mod utilities;
mod values;
mod variants;

pub use builtins::{
    builtin_declaration_formats, builtin_definitions, builtin_omit_token_prefixes,
    builtin_property_definitions, builtin_theme_order, builtin_utility_token_prefixes,
};
pub use utilities::{
    InvalidPropertyValue, NestedUtilityAtRule, NestedUtilityRule, ResolvedUtility,
    ResolvedUtilityTarget, UtilityArbitraryUse, UtilityFailure, UtilityFailureReason, UtilityOrder,
    UtilityPropertySort, UtilityResolutionError, UtilityTokenUse, check_candidate_variants,
    resolve_animation_option, resolve_utility,
};
pub use variants::{
    ConditionContext, ResolvedVariant, VariantApplication, VariantDeclaration, VariantFailure,
    VariantFailureReason, VariantMediaValue, VariantObstacle, VariantSequenceOrder,
    VariantValueSource, compare_variant_sequences, compatibility_marker_class,
    document_scoped_selector, is_tailwind_relationship_candidate, repeated_subject_selector,
    resolve_variant, resolve_variant_with_usage,
};

pub(crate) use programs::UtilityPrograms;
pub(crate) use values::color_fallback;

use context::{ArbitraryValueCheck, EvaluationContext, ValueSelections};

#[cfg(test)]
mod tests;
