//! Read-only access to the definitions selected by a snapshot.
//!
//! Every function takes the theme and returns a borrow of its registry, so evaluation
//! never needs a global registry or a process-wide default. The split between this
//! module and the theme index is deliberate: the index is the authored data, and this
//! module is the resolved view callers should use.

mod metadata;
mod registry;

pub use metadata::{
    builtin_declaration_formats, builtin_omit_token_prefixes, builtin_property_definitions,
    builtin_theme_order, builtin_utility_token_prefixes,
};
pub use registry::builtin_definitions;

pub(super) use metadata::{
    COMPAT_COLOR_LEGACY_SPACE, COMPAT_COLOR_MODERN_SPACE, COMPAT_COLOR_SUPPORT,
    builtin_marker_utilities, builtin_relationship_patterns, builtin_theme_namespace_types,
};
pub(super) use registry::{
    animation_option_property_mappings, arbitrary_property_order, builtin_animation_options,
    builtin_variants,
};
