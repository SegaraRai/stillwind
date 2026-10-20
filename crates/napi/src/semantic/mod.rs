//! Typed Node transport for semantic domain values; domain crates never depend on napi-rs.
//!
//! [`identity`] converts asset kinds and declaration sites; [`values`] checks symbolic
//! candidates and type facts; [`module_summary`] serializes module edges and exports.

mod identity;
mod module_summary;
mod values;

pub use identity::NativeIdentity;
pub use module_summary::{
    NativeCheckerBinding, NativeCheckerSpan, NativeLintComment, NativeModuleSummary,
    NativeTypeProbe,
};
pub use values::NativeTypeFact;
