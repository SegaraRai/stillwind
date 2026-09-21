//! Node-API surface for the Stillwind compiler.
//!
//! The binding is transport only. Every entry point converts JavaScript values into the
//! domain types of the Rust crates, calls the same services the CLI and the tests use,
//! and converts the result back; no rule, candidate, or registry decision is made here.
//!
//! Two families of functions exist. The one-shot functions ([`compile_source`],
//! [`compile_project`], [`render_stylesheet`], [`summarize_source`],
//! [`generate_vocabulary_types`]) are pure, so they have plain Rust counterparts that
//! the E2E tests call directly. The classes `NativeDesignSystemLoader` and
//! `NativeProjectSession` carry state across calls and are thin wrappers over
//! [`stillwind_design_system`] and [`stillwind_project`].
//!
//! File and module resolution stays in JavaScript. The binding only receives buffers
//! that the host already read, and it reports unresolved CSS imports as requests for
//! the host to satisfy.

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
mod naming;
mod options;
mod output;
mod semantic;
mod session;
mod source_input;
mod stylesheet;

pub use api::*;
pub use output::{
    BindingOutput, BindingRejectedIdentifier, ProjectBindingOutput, ProjectModuleOutput,
};
pub use session::NativeSessionError;
pub use source_input::ProjectSource;
