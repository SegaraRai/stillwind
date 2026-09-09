//! Oxc-backed JS/TS/JSX parsing and the first import-bound `style()` extractor.

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

#[cfg(test)]
mod tests;

pub use entry::*;
