//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.
//!
//! This crate is the JavaScript frontend. It parses a module with Oxc and finds the
//! `style` binding that a Stillwind macro import created. It extracts style calls, ref
//! definitions, stored variants, keyframes, and view-transition declarations, and
//! resolves the framework adapter. It then publishes a
//! [`stillwind_semantic::ModuleSummary`] and the type probes that the host checker must
//! answer.
//!
//! Three contracts hold throughout:
//!
//! - Extraction follows bindings, not names. A local `style` that shadows the import is
//!   not the macro.
//! - Every span in a result belongs to the editable original source, even when the
//!   input is a converter's shadow module.
//! - Scanning is demand-driven. Probes are created only for values that local analysis
//!   cannot resolve, so a module of literal utilities asks the checker nothing.
//!
//! Modules:
//!
//! - [`analyzer`] implements scanning and finalization.
//! - [`ast`] tags Oxc ranges with the identity of their analysis buffer and validates
//!   the Unicode strings that extraction reads.
//! - [`bindings`] recognizes imported macro symbols.
//! - [`contract`] validates macro argument expressions before folding.
//! - [`cva`] validates factory configurations and extracts their values.
//! - [`declarations`] collects ref and stored-variant declarations before the main
//!   visit.
//! - [`demand`] finds the imports that unresolved values need.
//! - [`entry`] coordinates parsing and extraction.
//! - [`expressions`] lowers style inputs.
//! - [`imports`] records bindings and their reads.
//! - [`keyframes`] extracts keyframes declarations and their uses.
//! - [`local_span`] supplies file-local range keys for extraction and checker
//!   bookkeeping.
//! - [`media`] records `style.media()` arguments.
//! - [`normalization`] validates the correspondence between authored expressions and
//!   converter code.
//! - [`projected_scan`] restores editable source coordinates for converted framework
//!   input.
//! - [`refs`] extracts ref declarations and their uses.
//! - [`resolution`] distinguishes unsupported values from missing facts.
//! - [`state`] holds the state of one extraction.
//! - [`stored_variants`] reads `style.when()` declarations and expands their uses.
//! - [`style_bindings`] seeds local style producers.
//! - [`stylesheet`] records `style.css()` inputs and options.
//! - [`templates`] preserves ref and stored-variant substitutions in template literals.
//! - [`uses`] rejects unconsumed projections and ref variants.
//! - [`view_transition`] extracts view-transition declarations and their uses.
//! - [`visit`] collects module structure and macro uses.

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
mod declarations;
mod demand;
mod entry;
mod expressions;
mod imports;
mod keyframes;
mod local_span;
mod media;
mod normalization;
mod projected_scan;
mod refs;
mod resolution;
mod state;
mod stored_variants;
mod style_bindings;
mod stylesheet;
mod templates;
mod uses;
mod view_transition;
mod visit;

pub use analyzer::OxcSourceAnalyzer;
pub use entry::{ParsedJavaScript, parse};
pub use normalization::{NormalizedProjection, normalize_projection};
