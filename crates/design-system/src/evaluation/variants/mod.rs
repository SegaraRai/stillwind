//! Variant resolution: turning one variant into the selector or condition it
//! contributes, and deciding the order of a variant stack.
//!
//! A variant is a small CSS program. [`body`] reads an authored definition body and
//! classifies it, [`substitution`] fills the capture and composes nested variants,
//! [`anchor`] finds the variant that selects the element of a document-level style,
//! [`negation`] negates a resolved application for `not-*`, [`resolve`] is the public
//! entry point, [`application`] defines its result and the queries over it,
//! [`compatibility`] answers questions about Tailwind spellings, [`conditions`] finds
//! redundant and contradicting variants of one candidate, and [`order`] compares two
//! resolved stacks using the registry's declared ordering rather than the order the
//! classes appear in markup. [`usage`] records direct token selections and viewport
//! value origins from the selected definition for source policies.
//!
//! Theme definitions remain immutable. Usage inspection creates an isolated recorder,
//! so resolving a stack repeatedly produces identical applications and usage records.

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
pub use compatibility::{compatibility_marker_class, is_tailwind_relationship_candidate};
pub use conditions::{
    RedundantVariant, UnsatisfiableVariants, VariantRedundancy, redundant_variants,
    unsatisfiable_variants,
};
pub use order::{VariantSequenceOrder, compare_variant_sequences};
pub use resolve::{ConditionContext, VariantFailure, VariantFailureReason, resolve_variant};
pub use usage::{
    ResolvedVariant, VariantMediaValue, VariantValueSource, resolve_variant_with_usage,
};
