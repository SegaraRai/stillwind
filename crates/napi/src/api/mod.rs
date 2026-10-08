//! One-shot compile entry points, kept independent of Node-API.
//!
//! The `*_input` functions take ordinary Rust values so the same behaviour can be
//! tested without a JavaScript runtime. The `#[napi]` functions next to them only
//! convert arguments, so a JavaScript caller and a Rust test exercise one
//! implementation.
//!
//! `compilation` owns standalone compilation and result conversion. `project`
//! compiles a prepared graph. `scan` exposes summaries and checker targets.
//! `stylesheet` renders shared CSS and folds constant expressions.

mod compilation;
mod project;
mod scan;
mod stylesheet;

pub use compilation::{
    BindingApiError, CompileCssInput, CompileSourceInput, NativeCompileCssInput,
    NativeCompileSourceInput, compile_source, compile_source_input,
};
pub use project::{compile_project, compile_project_input};
pub use scan::{scan_source, summarize_source, summarize_source_input};
pub use stylesheet::{fold_calc_expressions, render_stylesheet};

pub(crate) use compilation::binding_output_with_batch;
pub(crate) use scan::{SourceScan, scan_prepared};

use compilation::binding_output;
