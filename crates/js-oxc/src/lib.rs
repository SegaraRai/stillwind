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
//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.

mod ast;
mod bindings;
mod entry;
mod expressions;
mod motion;
mod refs;
mod state;
mod visit;

pub use entry::*;

#[cfg(test)]
mod tests;
