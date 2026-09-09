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
