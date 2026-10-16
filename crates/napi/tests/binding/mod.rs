//! End-to-end tests for the pure Rust entry points behind the Node binding.
//!
//! The binding itself is a thin conversion layer, so these tests drive
//! `compile_source_*`, [`summarize_source_input`], and [`render_stylesheet`] directly
//! and serialize results through serde. JSON
//! fields and native JavaScript properties use camelCase. These tests check
//! that projected runtime imports stay inside their verified script realms,
//! invalid projection metadata fails the compile instead of producing partial code, and
//! options are validated rather than defaulted silently.
//!
//! `compilation` checks serialized compilation, summaries, graph output, and options.
//! `naming` checks generated identities and rejected names in host output.
//! `reporting` checks lint severity and diagnostic editor coordinates.
//! `source_input` checks prepared document transport and projection contracts.
//! `support` supplies the builders shared by these cases.

use std::error::Error;

use stillwind_compiler::{CompilerOptions, IdentifierKind, RejectedIdentifier};
use stillwind_diagnostics::{
    CompileDiagnostic, LintOverrides, LintSeverity, RuleGroupInputErrorKind,
};
use stillwind_ir::{AdapterKind, ScriptLanguage, ScriptModuleKind};
use stillwind_napi::{
    BindingApiError, BindingRejectedIdentifier, CompileCssInput, CompileSourceInput,
    HostPreparedSource, compile_project_input, compile_source_input, render_stylesheet,
    summarize_source_input,
};
use stillwind_source::{ModuleId, SourceRegistry, SourceUri};
use stillwind_test_support::with_defaults;

mod compilation;
mod naming;
mod reporting;
mod source_input;
mod support;

use support::{compile_input, css_input, sources_from_value};
