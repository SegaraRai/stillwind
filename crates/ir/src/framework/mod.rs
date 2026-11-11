//! Framework policy, independent of parsers and host converters.
//!
//! Each entry point selects a shared output representation and a source projection
//! policy. [`AdapterKind::spec`] selects a specification, and [`FRAMEWORKS`] lists the
//! specifications in the order offered to hosts. Supporting another adapter requires
//! its specification and entries in the enum and the list, not a parser here. Source
//! parsing, projection bookkeeping, and host conversion belong to the frontend and host
//! layers.
//!
//! Modules:
//!
//! - [`alpine`] defines the macro policy for Alpine directives.
//! - [`astro`] defines the Astro adapter policy.
//! - [`lit`] defines the macro policy for Lit templates.
//! - [`output`] defines property names, style representations, and runtime modules for
//!   compiled props.
//! - [`preact`] defines the Preact adapter policy.
//! - [`projection`] defines the expressions and decisions that converter normalization
//!   uses.
//! - [`qwik`] defines the macro policy for Qwik JSX.
//! - [`react`] defines the React adapter policy.
//! - [`solid`] defines the Solid adapter policy.
//! - [`spec`] defines static framework specifications and adapter selection.
//! - [`svelte`] defines the Svelte adapter policy and bindings for helpers that
//!   svelte2tsx supplies.
//! - [`vanilla`] defines the macro policy for direct DOM consumers.
//! - [`vue`] defines the Vue adapter policy and its Volar projection rewrite.

mod alpine;
mod astro;
mod lit;
mod output;
mod preact;
mod projection;
mod qwik;
mod react;
mod solid;
mod spec;
mod svelte;
mod vanilla;
mod vue;

pub use output::{OutputFormat, OutputSpec, RUNTIME_IMPORT, RUNTIME_IMPORT_DEV, StyleFormat};
pub use projection::{ProjectionDecision, ProjectionExpression};
pub use spec::{AdapterKind, FRAMEWORKS, FrameworkSpec, MODULE_CSS_IMPORT, from_macro_import};

use projection::preserve;

#[cfg(test)]
mod tests;
