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
