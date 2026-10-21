//! Candidate evaluation: turning authored spellings into declarations and selectors
//! using only the definitions a snapshot holds.
//!
//! Entry points borrow the immutable [`crate::ThemeIndex`]. The theme owns effective
//! token lookup and parsed utility programs, which its clones and snapshots share.
//!
//! The modules below split the work by subject: [`stillwind`] reads the selected
//! registry, [`utilities`] matches and expands a utility body, [`values`] resolves
//! typed value domains and placeholders, and [`variants`] produces selector
//! applications and their ordering. [`context`] records direct selections for one
//! evaluation. [`programs`] prepares utility syntax and declaration branches.
//! [`suggestions`] proposes spellings for unknown utility, variant, and theme token
//! names.

mod context;
mod programs;
mod stillwind;
mod suggestions;
mod utilities;
mod values;
mod variants;

pub use stillwind::{
    stillwind_declaration_formats, stillwind_definitions, stillwind_omit_token_prefixes,
    stillwind_property_definitions, stillwind_theme_order, stillwind_utility_token_prefixes,
};
pub use suggestions::suggested_theme_token;
pub use utilities::{
    InvalidPropertyValue, InvalidValueReason, NestedUtilityAtRule, NestedUtilityRule,
    ResolvedUtility, ResolvedUtilityTarget, UntypedVariable, UtilityArbitraryUse,
    UtilityDiagnosticData, UtilityFailure, UtilityFailureReason, UtilityOrder, UtilityPropertySort,
    UtilityResolutionError, UtilityTokenUse, VariableMismatch, VariableTypeSource,
    check_candidate_variants, dynamic_named_spelling, resolve_utility, utility_value_range,
};
pub use values::{
    ThemeValueMismatch, named_domain_values, theme_value_mismatch, unresolvable_value_namespaces,
};
pub use variants::{
    ConditionContext, DocumentTargetFailure, MarkerUse, RedundantVariant, ResolvedVariant,
    UnsatisfiableVariants, VariantApplication, VariantContext, VariantDeclaration,
    VariantDiagnosticData, VariantFailure, VariantFailureReason, VariantMediaValue,
    VariantObstacle, VariantRedundancy, VariantSequenceOrder, VariantValueSource,
    compare_variant_sequences, compound_variant_inner, document_scoped_selector, document_target,
    exact_variant_definition_resolves, is_tailwind_relationship_candidate, marker_use,
    redundant_variants, repeated_subject_selector, resolve_variant, resolve_variant_with_usage,
    unsatisfiable_variants,
};

pub(crate) use programs::UtilityPrograms;
pub(crate) use values::color_fallback;

#[cfg(test)]
mod tests;
