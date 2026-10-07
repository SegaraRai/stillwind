//! The immutable snapshot: resolved semantics plus the lookup views built from them.
//!
//! Resolved definitions stay unchanged after construction. [`utility_search`] lazily
//! builds shared search metadata. A reload builds a new snapshot, and a request holds
//! one clone for its whole duration, so no caller observes a half-replaced registry.
//! [`data`] holds immutable evaluation data and registry views; [`registry`] indexes
//! definitions by name. [`analysis`] shares derived indexes whose keys include their
//! configuration. Each analysis is prepared at most once per revision and key.

mod analysis;
mod data;
mod registry;
mod utility_search;

pub use data::*;
pub use registry::*;
pub use utility_search::{UtilitySearchEntry, UtilitySearchIndex};
