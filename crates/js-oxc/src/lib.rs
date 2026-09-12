//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.
//!
//! This crate is the JavaScript frontend: it parses a module with Oxc, finds the
//! `style` binding that came from a Stillwind macro import, extracts style calls,
//! handle definitions, animation and view-transition declarations, resolves the
//! framework adapter, and publishes a [`stillwind_semantic::ModuleSummary`] plus the
//! type probes the host checker still has to answer.
//!
//! Three contracts hold throughout. Extraction is binding-bound, never name-bound: a
//! local `style` that shadows the import is not the macro. Every span in a result
//! belongs to the editable original source, even when the input was a converter's
//! shadow module. And scanning is demand-driven: probes are created only for values
//! that local analysis could not resolve, so a module of literal utilities asks the
//! checker nothing.

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
