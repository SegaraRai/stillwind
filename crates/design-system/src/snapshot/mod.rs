//! The immutable snapshot: resolved semantics and the lookup views built from them.
//!
//! Resolved definitions stay unchanged after construction. A reload builds a new
//! snapshot, and a request holds one clone for its whole duration, so no caller
//! observes a half-replaced registry.
//!
//! Modules:
//!
//! - [`analysis`] shares derived indexes whose keys include their configuration. Each
//!   analysis is prepared at most once per revision and key.
//! - [`data`] holds immutable evaluation data and registry views.
//! - [`registry`] indexes definitions by name.
//! - [`utility_search`] builds shared search metadata on first use.

mod analysis;
mod data;
mod registry;
mod utility_search;

pub use data::{
    DefinitionFingerprints, DesignSystemError, DesignSystemRevision, DesignSystemSnapshot,
};
pub use registry::{
    DefinitionRegistry, PropertyRegistry, ThemeRegistry, UtilityRegistry, VariantRegistry,
};
pub use utility_search::{
    SplitFamily, UtilitySearchEntry, UtilitySearchIndex, declared_properties,
};
