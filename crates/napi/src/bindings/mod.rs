//! Node-API surface for running the Stillwind compiler inside Vite.

mod session;

use std::collections::{BTreeMap, BTreeSet};

use napi::{Error, Result};
use napi_derive::napi;
use serde::{Deserialize, Serialize};
use stillwind_core::{CompileResult, Compiler, CompilerOptions, GeneratedClass, GeneratedSymbol};
use stillwind_core::{
    DesignSystemSnapshot, Fingerprint, ModuleCompiler, OxcSourceAnalyzer, SourceAnalyzer, TypeFacts,
};
use stillwind_semantic::{AssetMetadata, IdentityId, TypeFact};
use stillwind_source::{FileId, SourceFile};

#[derive(Serialize)]
struct BindingOutput {
    css: String,
    code: Option<String>,
    mappings: Vec<BindingMapping>,
    classes: Vec<GeneratedClass>,
    symbols: Vec<GeneratedSymbol>,
    diagnostics: Vec<BindingDiagnostic>,
    lint_findings: Vec<BindingLintFinding>,
    lint_policy: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
struct BindingMapping {
    original_start: u32,
    original_end: u32,
    generated_start: u32,
    generated_end: u32,
}

#[derive(Deserialize)]
struct ProjectSource {
    path: String,
    source: String,
    #[serde(default)]
    type_facts: Vec<TypeFact>,
}

#[derive(Serialize)]
struct ProjectBindingOutput {
    modules: Vec<ProjectModuleOutput>,
}

#[derive(Serialize)]
struct ProjectModuleOutput {
    path: String,
    #[serde(flatten)]
    output: BindingOutput,
}

#[derive(Serialize)]
struct BindingDiagnostic {
    code: String,
    message: String,
    file: String,
    start: u32,
    end: u32,
    start_utf16: u32,
    end_utf16: u32,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct BindingLintFinding {
    rule: String,
    message: String,
    file: String,
    start: u32,
    end: u32,
    notes: Vec<String>,
}

mod api;
pub use api::*;
use api::{binding_output, parse_options};
