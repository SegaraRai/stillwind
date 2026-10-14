//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.
//!
//! This crate is the JavaScript frontend: it parses a module with Oxc, finds the
//! `style` binding that came from a Stillwind macro import, extracts style calls,
//! ref definitions, keyframes and view-transition declarations, resolves the framework
//! adapter, and publishes a [`stillwind_semantic::ModuleSummary`] plus the type probes
//! the host checker still has to answer. `entry` coordinates parsing and extraction;
//! `contract` validates macro argument expressions before folding; `uses` rejects
//! unconsumed projections and ref variants. `normalization` and `projected_scan`
//! restore editable source coordinates for converted framework input, including the
//! effect-proof spans. `local_span` supplies file-local range keys for extraction and
//! checker bookkeeping.
//!
//! Three contracts hold throughout. Extraction is binding-bound, never name-bound: a
//! local `style` that shadows the import is not the macro. Every span in a result
//! belongs to the editable original source, even when the input was a converter's
//! shadow module. And scanning is demand-driven: probes are created only for values
//! that local analysis could not resolve, so a module of literal utilities asks the
//! checker nothing.
//!
//! `analyzer` implements scanning and finalization. `state` holds one extraction, and
//! `visit` collects module structure and macro uses. `bindings` recognizes imported
//! macro symbols, and `style_bindings` seeds local producers. `imports` records
//! bindings and their reads. `cva` validates factory configurations and extracts their
//! values. `expressions` lowers style inputs, and `templates` preserves ref
//! substitutions. `resolution` distinguishes unsupported values from missing facts.
//! `refs`, `keyframes`, and `view_transition` extract their declarations
//! and uses. `demand` finds the imports needed by unresolved values. `ast` tags Oxc
//! ranges with their analysis buffer identity before coordinate mapping.

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
mod contract;
mod cva;
mod demand;
mod entry;
mod expressions;
mod imports;
mod keyframes;
mod local_span;
mod normalization;
mod projected_scan;
mod refs;
mod resolution;
mod state;
mod style_bindings;
mod templates;
mod uses;
mod view_transition;
mod visit;

pub use analyzer::OxcSourceAnalyzer;
pub use entry::{ParsedJavaScript, parse};
pub use normalization::{NormalizedProjection, normalize_projection};
