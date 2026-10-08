//! Framework policy, independent of parsers and host converters.
//!
//! Each framework states the same facts: which module exports the macro, which one
//! provides the runtime, what the class attribute is called, how the runtime
//! materializes the style payload, and whether the framework's converter needs
//! original-source projection.
//!
//! `spec` owns adapter selection and shared specifications. `projection` defines the
//! expressions and decisions used by converter normalization.
//! The `astro`, `preact`, `react`, `solid`, `svelte`, and `vue` modules supply static
//! specifications and expression-rewrite policies. [`AdapterKind::spec`] selects a
//! specification; [`FRAMEWORKS`] lists them in the order offered to hosts. Supporting
//! another adapter requires its specification and enum/list entries, not a parser here.
//! Source parsing, projection bookkeeping, and host conversion belong to the frontend
//! and host layers.

mod astro;
mod preact;
mod projection;
mod react;
mod solid;
mod spec;
mod svelte;
mod vue;

pub use projection::{ProjectionDecision, ProjectionExpression};
pub use spec::{
    AdapterKind, FRAMEWORKS, FrameworkSpec, MODULE_CSS_IMPORT, StyleFormat, from_macro_import,
};

use projection::preserve;

#[cfg(test)]
mod tests;
