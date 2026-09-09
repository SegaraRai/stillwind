use lightningcss::{
    printer::PrinterOptions,
    stylesheet::{ParserOptions, StyleSheet},
};
use proptest::prelude::*;
use std::{fs, path::Path};
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
