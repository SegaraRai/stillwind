//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.

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

mod ast;
mod bindings;
mod demand;
mod entry;
mod expressions;
mod motion;
mod refs;
mod state;
mod visit;

pub use entry::*;

#[cfg(test)]
mod tests;
