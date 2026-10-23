//! Framework policy, independent of parsers and host converters.
//!
//! Each entrypoint selects a shared output representation and a source projection
//! policy. `output` owns property names, style representations, and runtime modules.
//! Framework converter policies normalize original-source projections.
//!
//! `spec` owns adapter selection and shared specifications. `projection` defines the
//! expressions and decisions used by converter normalization.
//! The framework modules and `vanilla` supply static
//! specifications and expression-rewrite policies. [`AdapterKind::spec`] selects a
//! specification; [`FRAMEWORKS`] lists them in the order offered to hosts. Supporting
//! another adapter requires its specification and enum/list entries, not a parser here.
//! Source parsing, projection bookkeeping, and host conversion belong to the frontend
//! and host layers.

mod astro;
mod output;
mod preact;
mod projection;
mod react;
mod solid;
mod spec;
mod svelte;
mod vanilla;
mod vue;

pub use output::{OutputFormat, OutputSpec, StyleFormat};
pub use projection::{ProjectionDecision, ProjectionExpression};
pub use spec::{AdapterKind, FRAMEWORKS, FrameworkSpec, MODULE_CSS_IMPORT, from_macro_import};

use projection::preserve;

#[cfg(test)]
mod tests;
