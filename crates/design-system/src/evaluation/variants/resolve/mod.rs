//! Public variant resolution.
//!
//! Resolution answers what one variant contributes to a selector, as a
//! [`VariantApplication`]. Project definitions are consulted before built-ins. A
//! variant that resolves to nothing is not an error: [`resolve_variant`] returns a
//! [`VariantFailure`] that names the cause, and the caller decides how to report it.
//!
//! [`variant`] resolves one variant and defines the failures that resolution reports.
//! [`arbitrary`] resolves `[...]` variant text and builds inner variants from captures.
//! [`definition`] evaluates one registered definition for a capture, including typed
//! captures, modifiers, and nested variants.

mod arbitrary;
mod definition;
mod variant;

pub use definition::exact_variant_definition_resolves;
pub use variant::{ConditionContext, VariantFailure, VariantFailureReason, resolve_variant};

pub(in crate::evaluation) use arbitrary::{
    decode_variant_template, parse_nested_variant, relative_selector,
};
pub(in crate::evaluation) use definition::{
    resolve_named_variant_with, resolve_registered_variant_candidate, split_typed_capture,
    split_variant_modifier,
};
