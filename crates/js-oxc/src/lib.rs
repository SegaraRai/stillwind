//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.

mod definitions;
mod entry;
mod expressions;
mod labels;
mod motion;
mod projections;
mod references;
mod state;
mod validation;
mod visit;

pub use entry::*;

#[cfg(test)]
mod tests;
