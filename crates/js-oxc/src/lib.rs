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

mod analyzer;
mod ast;
mod bindings;
mod demand;
mod entry;
mod expressions;
mod imports;
mod motion;
mod normalization;
mod projected_scan;
mod refs;
mod state;
mod style_bindings;
mod templates;
mod visit;

pub use analyzer::OxcSourceAnalyzer;
pub use entry::{ParsedJavaScript, parse, parse_with_language, parse_with_type_facts};
pub use normalization::{NormalizedProjection, normalize_projection};
