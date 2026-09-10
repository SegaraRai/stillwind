use std::collections::BTreeMap;

use serde::Serialize;

pub use stillwind_compiler::TransformMapping as BindingMapping;
use stillwind_compiler::{GeneratedClass, GeneratedSymbol, ModuleResourceReference};

#[derive(Serialize)]
pub struct BindingOutput {
    pub resources: Vec<ModuleResourceReference>,
    pub css: String,
    pub code: Option<String>,
    pub mappings: Vec<BindingMapping>,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub diagnostics: Vec<BindingDiagnostic>,
    pub lint_findings: Vec<BindingLintFinding>,
    pub lint_policy: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct BindingDiagnostic {
    pub code: String,
    pub message: String,
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub start_utf16: u32,
    pub end_utf16: u32,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub notes: Vec<String>,
}

#[derive(Serialize)]
pub struct BindingLintFinding {
    pub rule: String,
    pub message: String,
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub notes: Vec<String>,
}
