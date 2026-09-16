//! Variant resolution: turning one variant into the selector or condition it
//! contributes, and deciding the order of a variant stack.
//!
//! A variant is a small CSS program. [`body`] reads an authored definition body and
//! classifies it, [`substitution`] fills the capture and composes nested variants,
//! [`resolve`] is the public entry point, and [`order`] compares two resolved stacks
//! using the registry's declared ordering rather than the order the classes appear in
//! markup.
//!
//! Nothing here mutates the theme or caches a resolution, so a stack may be resolved
//! repeatedly with identical results.

pub(super) mod body;
pub(super) mod order;
pub(super) mod resolve;
pub(super) mod substitution;

pub use order::compare_variant_sequences;
pub use resolve::{
    VariantApplication, VariantDeclaration, compatibility_marker_class, document_scoped_selector,
    is_condition_variant, is_tailwind_relationship_candidate, resolve_variant,
};
