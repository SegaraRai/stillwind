//! End-to-end compilation of TSX sources against real themes.
//!
//! The suite drives the public compiler APIs with the Oxc frontend and checks the
//! emitted CSS and the transformed source together.
//!
//! Modules:
//!
//! - [`acceptance`] checks the acceptance cases of the compatibility slice.
//! - [`conditional_flow`] checks the structure-preserving lowering of conditional class
//!   expressions.
//! - [`css_call`] checks `style.css()` lowering.
//! - [`cva`] checks CVA extraction, style lowering, and runtime imports.
//! - [`definitions`] checks animation and view-transition compilation.
//! - [`folding`] checks fragment layouts, macro contracts, and source attribution.
//! - [`media`] checks `style.media()` lowering.
//! - [`numeric`] runs property tests over numeric candidates.
//! - [`parity`] compares output with the checked-in Tailwind v4 fixtures.
//! - [`react`] checks the React adapter.
//! - [`ref_variants`] checks ref variants in selectors and templates.
//! - [`refs`] checks ref projections and selector scopes.
//! - [`runtime_if`] checks experimental computed-value conditions.
//! - [`support`] compiles cases and compares stylesheets for the other modules.
//! - [`table_lookup`] checks lookups in array and object literals.
//! - [`theme`] checks theme emission and token gating.
//! - [`utilities`] checks utility resolution and its emitted CSS.
//! - [`variants`] checks variant lowering and the utilities layer.

use std::fs;
use std::path::Path;

use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use proptest::prelude::*;

use stillwind_compiler::Compiler;
use stillwind_source::{FileId, SourceFile};

mod support;
use support::*;

mod acceptance;
mod conditional_flow;
mod css_call;
mod cva;
mod definitions;
mod folding;
mod media;
mod numeric;
mod parity;
mod react;
mod ref_variants;
mod refs;
mod runtime_if;
mod table_lookup;
mod theme;
mod utilities;
mod variants;
