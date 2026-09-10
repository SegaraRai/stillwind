use std::{fs, path::Path};

use lightningcss::{
    printer::PrinterOptions,
    stylesheet::{ParserOptions, StyleSheet},
};
use proptest::prelude::*;

use stillwind_core::Compiler;
use stillwind_source::{FileId, SourceFile};

mod support;
use support::*;

mod acceptance;
mod motion;
mod numeric;
mod parity;
mod react;
mod refs;
mod theme;
mod utilities;
mod variants;

#[path = "../../../test-support/css.rs"]
mod css_fixture;
