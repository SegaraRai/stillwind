pub(super) mod body;
pub(super) mod order;
pub(super) mod resolve;
pub(super) mod substitution;

pub use order::compare_variant_sequences;
pub use resolve::{
    VariantApplication, VariantDeclaration, compatibility_marker_class,
    is_tailwind_relationship_candidate, resolve_variant,
};
