//! Serializable compiler output.
//!
//! [`BindingOutput`] carries module results from Rust services to JavaScript adapters.
//! [`BindingReport`] carries the document identity, revision, complete editor range,
//! related highlights, and fix actions of the common reporting layer. The compatibility
//! diagnostic and lint records also carry primary coordinates. Other records document
//! their coordinate units on their fields.
//!
//! Generated code is absent when compiler diagnostics are present. These types describe
//! one result; the host session is responsible for keeping an earlier successful
//! result.
//!
//! Modules:
//!
//! - [`compilation`] carries compiler results and generated identities.
//! - [`mapping`] carries original and transformed source coordinates.
//! - [`reporting`] carries diagnostic batches and scan outcomes.
//! - [`text`] transfers owned UTF-8 buffers to Node-API.

mod compilation;
mod mapping;
mod reporting;
mod text;

pub use compilation::{
    BindingOutput, BindingRejectedIdentifier, NativeModuleResource, ProjectBindingOutput,
    ProjectModuleOutput,
};
pub use mapping::{NativeSpan, NativeTransformMapping};
pub use reporting::{
    BindingDiagnostic, BindingLintFinding, BindingReport, NativeDiagnosticBatch, NativeScanResult,
};
pub use text::NativeText;

pub(crate) use reporting::report_error;
