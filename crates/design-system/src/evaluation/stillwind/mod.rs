//! Read-only access to the definitions selected by a snapshot.
//!
//! [`registry`] borrows utility, variant, and animation definitions from the supplied
//! theme. [`metadata`] reads declaration formats, property registrations, and ordering
//! metadata, and derives token prefixes for completion. Both use the caller's theme
//! without consulting a global registry.

mod metadata;
mod registry;

pub use metadata::{
    stillwind_declaration_formats, stillwind_omit_token_prefixes, stillwind_property_definitions,
    stillwind_theme_order, stillwind_utility_token_prefixes,
};
pub use registry::stillwind_definitions;

pub(super) use metadata::{
    COMPAT_COLOR_MODERN_SPACE, COMPAT_COLOR_SUPPORT, stillwind_markers,
    stillwind_relationship_patterns,
};
pub(super) use registry::{arbitrary_property_order, stillwind_variants};
