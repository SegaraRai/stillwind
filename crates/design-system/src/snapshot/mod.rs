//! The immutable snapshot: resolved semantics plus the lookup views built from them.
//!
//! Nothing in this module mutates a snapshot after construction. A reload builds a new
//! value, and a request holds one clone for its whole duration, so no caller can
//! observe a half-replaced registry.

mod data;
mod registry;

pub use data::*;
pub use registry::*;
