//! Serializable compiler output.
//!
//! [`BindingOutput`] carries module results from Rust services to JavaScript adapters.
//! [`BindingReport`] carries the common reporting layer's document identity, revision,
//! complete editor range, related highlights, and fix actions. The compatibility
//! diagnostic and lint records carry primary coordinates too. Other records document
//! their own coordinate units at the field level.
//!
//! Generated code is absent when compiler diagnostics are present. These types describe
//! one result; retaining an earlier successful result is the host session's responsibility.
//!
//! `compilation` carries compiler results and generated identities. `reporting`
//! carries diagnostic batches and scan outcomes. `mapping` carries original and
//! transformed source coordinates.

mod compilation;
mod mapping;
mod reporting;

pub(crate) use reporting::report_error;

pub use compilation::{
    BindingOutput, BindingRejectedIdentifier, NativeModuleResource, ProjectBindingOutput,
    ProjectModuleOutput,
};
pub use mapping::{NativeSpan, NativeTransformMapping};
pub use reporting::{
    BindingDiagnostic, BindingLintFinding, BindingReport, NativeDiagnosticBatch, NativeScanResult,
};
