//! Variant resolution: turning one variant into the selector or condition that it
//! contributes, and deciding the order of a variant stack.
//!
//! A variant is a small CSS program. Theme definitions stay immutable, and usage
//! inspection creates an isolated recorder, so resolving a stack repeatedly produces
//! identical applications and usage records.
//!
//! Modules:
//!
//! - [`anchor`] finds the variant that selects the element of a document-level style.
//! - [`application`] defines the result of resolution and the queries over it.
//! - [`body`] reads an authored definition body and classifies it.
//! - [`compatibility`] answers questions about Tailwind spellings.
//! - [`conditions`] finds redundant and contradicting variants of one candidate.
//! - [`negation`] negates a resolved application for `not-*`.
//! - [`order`] compares two resolved stacks by the order that the registry declares,
//!   not by the order of classes in markup.
//! - [`resolve`] is the public entry point.
//! - [`substitution`] fills the capture and composes nested variants.
//! - [`usage`] records direct token selections and viewport value origins from the
//!   selected definition for source policies.

mod anchor;
mod application;
mod body;
mod compatibility;
mod conditions;
mod negation;
mod order;
mod resolve;
mod substitution;
mod usage;

pub use anchor::{DocumentTargetFailure, VariantContext, document_target};
pub use application::{
    VariantApplication, VariantDeclaration, VariantObstacle, document_scoped_selector,
    repeated_subject_selector,
};
pub use compatibility::{MarkerUse, is_tailwind_relationship_candidate, marker_use};
pub use conditions::{
    RedundantVariant, UnsatisfiableVariants, VariantRedundancy, redundant_variants,
    unsatisfiable_variants,
};
pub use order::{VariantSequenceOrder, compare_variant_sequences, compound_variant_inner};
pub use resolve::{
    ConditionContext, VariantFailure, VariantFailureReason, exact_variant_definition_resolves,
    resolve_variant, variant_alternatives,
};
pub use usage::{
    ResolvedVariant, VariantDiagnosticData, VariantMediaValue, VariantValueSource,
    resolve_variant_with_usage,
};
