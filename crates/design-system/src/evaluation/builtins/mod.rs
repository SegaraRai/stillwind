//! Read-only access to the definitions selected by a snapshot.
//!
//! Every function takes the theme and returns a borrow of its registry, so evaluation
//! never needs a global registry or a process-wide default. The split between this
//! module and the theme index is deliberate: the index is the authored data, and this
//! module is the resolved view callers should use.

pub(super) mod metadata;
pub(super) mod registry;

pub use metadata::{
    builtin_declaration_formats, builtin_omit_token_prefixes, builtin_property_definitions,
    builtin_theme_order, builtin_utility_token_prefixes,
};
pub use registry::builtin_definitions;
