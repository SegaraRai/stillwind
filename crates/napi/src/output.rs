use std::collections::BTreeMap;

use napi_derive::napi;
use serde::Serialize;

use stillwind_compiler::{
    GeneratedClass, GeneratedSymbol, ModuleResourceReference, TransformMapping,
};
use stillwind_ir::StyleDependency;
use stillwind_source::Span;

use crate::semantic::NativeIdentity;

#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct BindingOutput {
    pub composition: Vec<NativeStyleDependency>,
    pub resources: Vec<NativeModuleResource>,
    pub css: String,
    pub code: Option<String>,
    pub mappings: Vec<NativeTransformMapping>,
    pub classes: Vec<NativeGeneratedClass>,
    pub symbols: Vec<NativeGeneratedSymbol>,
    pub diagnostics: Vec<BindingDiagnostic>,
    #[napi(js_name = "lint_findings")]
    pub lint_findings: Vec<BindingLintFinding>,
    #[napi(
        js_name = "lint_policy",
        ts_type = "Record<string, 'warn' | 'deny' | 'off'>"
    )]
    pub lint_policy: BTreeMap<String, String>,
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeStyleDependency {
    pub derived: NativeIdentity,
    pub base: NativeIdentity,
}

impl From<StyleDependency> for NativeStyleDependency {
    fn from(value: StyleDependency) -> Self {
        Self {
            derived: value.derived.into(),
            base: value.base.into(),
        }
    }
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct BindingDiagnostic {
    pub code: String,
    pub message: String,
    pub file: String,
    pub start: u32,
    pub end: u32,
    #[napi(js_name = "start_utf16")]
    pub start_utf16: u32,
    #[napi(js_name = "end_utf16")]
    pub end_utf16: u32,
    #[napi(js_name = "start_line")]
    pub start_line: u32,
    #[napi(js_name = "start_column")]
    pub start_column: u32,
    #[napi(js_name = "end_line")]
    pub end_line: u32,
    #[napi(js_name = "end_column")]
    pub end_column: u32,
    pub notes: Vec<String>,
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct BindingLintFinding {
    pub rule: String,
    pub message: String,
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub notes: Vec<String>,
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeModuleResource {
    pub placeholder: String,
    pub owner: String,
    pub specifier: String,
}

impl From<ModuleResourceReference> for NativeModuleResource {
    fn from(value: ModuleResourceReference) -> Self {
        Self {
            placeholder: value.placeholder,
            owner: value.owner,
            specifier: value.specifier,
        }
    }
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeTransformMapping {
    #[napi(js_name = "original_start")]
    pub original_start: u32,
    #[napi(js_name = "original_end")]
    pub original_end: u32,
    #[napi(js_name = "generated_start")]
    pub generated_start: u32,
    #[napi(js_name = "generated_end")]
    pub generated_end: u32,
}

impl From<TransformMapping> for NativeTransformMapping {
    fn from(value: TransformMapping) -> Self {
        Self {
            original_start: value.original_start,
            original_end: value.original_end,
            generated_start: value.generated_start,
            generated_end: value.generated_end,
        }
    }
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeSpan {
    pub file: u32,
    pub start: u32,
    pub end: u32,
}

impl From<Span> for NativeSpan {
    fn from(value: Span) -> Self {
        Self {
            file: value.file.0,
            start: value.start.0,
            end: value.end.0,
        }
    }
}

#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeGeneratedClass {
    #[napi(js_name = "class_name")]
    pub class_name: String,
    #[napi(js_name = "call_span")]
    pub call_span: NativeSpan,
    pub guard: Option<NativeSpan>,
    #[napi(js_name = "guard_when_true")]
    pub guard_when_true: bool,
}

impl From<GeneratedClass> for NativeGeneratedClass {
    fn from(value: GeneratedClass) -> Self {
        Self {
            class_name: value.class_name,
            call_span: value.call_span.into(),
            guard: value.guard.map(Into::into),
            guard_when_true: value.guard_when_true,
        }
    }
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeGeneratedSymbol {
    pub name: String,
    pub kind: String,
    pub identity: String,
    #[napi(js_name = "source_path")]
    pub source_path: String,
    pub start: u32,
    pub end: u32,
}

impl From<GeneratedSymbol> for NativeGeneratedSymbol {
    fn from(value: GeneratedSymbol) -> Self {
        Self {
            name: value.name,
            kind: value.kind,
            identity: value.identity,
            source_path: value.source_path,
            start: value.start,
            end: value.end,
        }
    }
}

#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct ProjectBindingOutput {
    pub modules: Vec<ProjectModuleOutput>,
}

#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct ProjectModuleOutput {
    pub composition: Vec<NativeStyleDependency>,
    pub path: String,
    pub resources: Vec<NativeModuleResource>,
    pub css: String,
    pub code: Option<String>,
    pub mappings: Vec<NativeTransformMapping>,
    pub classes: Vec<NativeGeneratedClass>,
    pub symbols: Vec<NativeGeneratedSymbol>,
    pub diagnostics: Vec<BindingDiagnostic>,
    #[napi(js_name = "lint_findings")]
    pub lint_findings: Vec<BindingLintFinding>,
    #[napi(
        js_name = "lint_policy",
        ts_type = "Record<string, 'warn' | 'deny' | 'off'>"
    )]
    pub lint_policy: BTreeMap<String, String>,
}

impl ProjectModuleOutput {
    pub(crate) fn new(path: String, output: BindingOutput) -> Self {
        let BindingOutput {
            composition,
            resources,
            css,
            code,
            mappings,
            classes,
            symbols,
            diagnostics,
            lint_findings,
            lint_policy,
        } = output;
        Self {
            composition,
            path,
            resources,
            css,
            code,
            mappings,
            classes,
            symbols,
            diagnostics,
            lint_findings,
            lint_policy,
        }
    }
}
