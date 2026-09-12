//! End-to-end compilation of TSX sources against real themes.
//!
//! The suite drives the public `Compiler` API with the Oxc frontend and asserts on the emitted CSS
//! and the transformed source together. `support` holds the fixture readers and CSS comparison
//! helpers shared by the modules below; `parity` and `theme` also compare against the checked-in
//! Tailwind v4 fixtures.

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
mod motion;
mod numeric;
mod parity;
mod react;
mod ref_variants;
mod refs;
mod theme;
mod utilities;
mod variants;

#[path = "../../../test-support/css.rs"]
mod css_fixture;
