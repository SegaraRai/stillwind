//! Node-API surface for the Stillwind compiler.
//!
//! The binding only transports data. Every entry point converts JavaScript values into
//! the domain types of the Rust crates, calls the same services that the CLI and the
//! tests use, and converts the result back. It makes no rule, candidate, or registry
//! decision.
//!
//! The one-shot functions ([`compile_source`], [`compile_project`],
//! [`render_stylesheet`], [`summarize_source`], and [`minify_css`]) are pure, so the
//! E2E tests call their plain Rust counterparts directly. The classes
//! `NativeDesignSystemLoader` and `NativeProjectSession` in [`session`] keep state
//! across calls and wrap [`stillwind_design_system`] and [`stillwind_project`].
//!
//! File and module resolution stays in JavaScript. The binding receives only buffers
//! that the host has already read, and it reports unresolved CSS imports as requests
//! for the host.
//!
//! Modules:
//!
//! - [`api`] provides the one-shot operations.
//! - [`editor`] answers editor queries and reports lint findings.
//! - [`mod@fingerprint`] exposes content identities.
//! - [`minify`] transports final CSS minification.
//! - [`module_editor`] captures source occurrences and previews from a finalized
//!   analysis.
//! - [`naming`] calls host name generators.
//! - [`options`] converts compiler options.
//! - [`output`] serializes compiler results.
//! - [`semantic`] converts checker facts and module summaries.
//! - [`session`] wraps the stateful loader and project session.
//! - [`source_input`] validates the transport of documents and projections.
//! - [`stylesheet`] carries CSS load requests and rendered CSS.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod api;
mod editor;
mod fingerprint;
mod minify;
mod module_editor;
mod naming;
mod options;
mod output;
mod semantic;
mod session;
mod source_input;
mod stylesheet;

pub use api::{
    BindingApiError, CompileCssInput, CompileSourceInput, NativeCompileCssInput,
    NativeCompileSourceInput, compile_project, compile_project_input, compile_source,
    compile_source_input, fold_calc_expressions, render_stylesheet, scan_source, summarize_source,
    summarize_source_input,
};
pub use editor::{
    NativeStylesheetPluginPath, completion_detail, decode_arbitrary_value, editor_css_source,
    stylesheet_plugin_paths,
};
pub use fingerprint::fingerprint;
pub use minify::{NativeCssMinifyOptions, NativeMinifiedCss, minify_css};
pub use output::{
    BindingOutput, BindingRejectedIdentifier, ProjectBindingOutput, ProjectModuleOutput,
};
pub use session::NativeSessionError;
pub use source_input::{HostPreparedSource, HostSourceDocument, HostSourceProjection};
