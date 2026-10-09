//! Node-API surface for the Stillwind compiler.
//!
//! The binding is transport only. Every entry point converts JavaScript values into the
//! domain types of the Rust crates, calls the same services the CLI and the tests use,
//! and converts the result back; no rule, candidate, or registry decision is made here.
//!
//! Two families of functions exist. The one-shot functions ([`compile_source`],
//! [`compile_project`], [`render_stylesheet`], [`summarize_source`], [`minify_css`])
//! are pure, so they have plain Rust counterparts that the E2E tests call directly. The
//! classes `NativeDesignSystemLoader` and `NativeProjectSession` carry state across
//! calls and are thin wrappers over [`stillwind_design_system`] and
//! [`stillwind_project`].
//!
//! File and module resolution stays in JavaScript. The binding only receives buffers
//! that the host already read, and it reports unresolved CSS imports as requests for
//! the host to satisfy.
//!
//! `api` provides standalone operations, and `session` wraps stateful loaders and
//! project sessions. `source_input` validates document and projection transport.
//! `semantic` converts checker facts and module summaries, while `options` converts
//! configuration and `naming` invokes host name generators. `output` serializes
//! compiler results, and `stylesheet` carries load requests and rendered CSS. `editor`
//! handles editor queries and lint reporting. `module_editor` captures source
//! occurrences and previews from finalized analysis. `fingerprint` exposes content
//! identities, and `minify` transports final CSS minification.

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
pub use fingerprint::fingerprint;
pub use minify::{NativeCssMinifyOptions, NativeMinifiedCss, minify_css};
pub use output::{
    BindingOutput, BindingRejectedIdentifier, ProjectBindingOutput, ProjectModuleOutput,
};
pub use session::NativeSessionError;
pub use source_input::{HostPreparedSource, HostSourceDocument, HostSourceProjection};
