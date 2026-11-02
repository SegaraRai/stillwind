//! Typed Node transport for semantic domain values, so that the domain crates never
//! depend on napi-rs.
//!
//! Modules:
//!
//! - [`identity`] converts asset kinds and declaration sites.
//! - [`module_summary`] serializes module edges and exports.
//! - [`values`] checks symbolic candidates and type facts.

mod identity;
mod module_summary;
mod values;

pub use identity::NativeIdentity;
pub use module_summary::{
    NativeCheckerBinding, NativeCheckerSpan, NativeLintComment, NativeModuleSummary,
    NativeTypeProbe,
};
pub use values::NativeTypeFact;
