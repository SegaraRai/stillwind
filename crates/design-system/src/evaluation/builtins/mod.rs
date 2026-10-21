//! Read-only access to the definitions selected by a snapshot.
//!
//! [`registry`] borrows utility, variant, and animation definitions from the supplied
//! theme. [`metadata`] reads declaration formats, property registrations, and ordering
//! metadata, and derives token prefixes for completion. Both use the caller's theme
//! without consulting a global registry.

mod metadata;
mod registry;

pub use metadata::{
    builtin_declaration_formats, builtin_omit_token_prefixes, builtin_property_definitions,
    builtin_theme_order, builtin_utility_token_prefixes,
};
pub use registry::builtin_definitions;

pub(super) use metadata::{
    COMPAT_COLOR_MODERN_SPACE, COMPAT_COLOR_SUPPORT, builtin_markers, builtin_relationship_patterns,
};
pub(super) use registry::{arbitrary_property_order, builtin_variants};
