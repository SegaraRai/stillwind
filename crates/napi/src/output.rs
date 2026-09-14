//! Serializable compiler output.
//!
//! [`BindingOutput`] carries module results from Rust services to JavaScript adapters.
//! [`BindingDiagnostic`] publishes byte ranges and UTF-16 offsets with zero-based
//! line/column coordinates. [`BindingLintFinding`] publishes byte ranges only; consumers
//! needing editor coordinates must convert those ranges against the referenced source.
//! Other records document their own coordinate units at the field level.
//!
//! Generated code is absent when compiler diagnostics are present. These types describe
//! one result; retaining an earlier successful result is the host session's responsibility.

use std::collections::BTreeMap;

use napi_derive::napi;
use serde::Serialize;

use stillwind_compiler::{
    ExpressionTransformMapping, GeneratedClass, GeneratedSymbol, GuardStep,
    ModuleResourceReference, TransformMapping,
};
use stillwind_ir::StyleDependency;
use stillwind_source::Span;

use crate::semantic::NativeIdentity;

/// Result of compiling one module.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct BindingOutput {
    /// Composition edges this module declared or consumed.
    pub composition: Vec<NativeStyleDependency>,
    /// Assets and resources the module references.
    pub resources: Vec<NativeModuleResource>,
    /// Rendered CSS for this compilation; standalone entry points include document CSS.
    pub css: String,
    /// Completely transformed source, or `None` when compilation failed.
    pub code: Option<String>,
    /// Mappings from the transformed source back to the original.
    pub mappings: Vec<NativeTransformMapping>,
    /// Class names generated for this module's calls.
    pub classes: Vec<NativeGeneratedClass>,
    /// Generated identifiers and their identities.
    pub symbols: Vec<NativeGeneratedSymbol>,
    /// Errors. Non-empty means nothing was published for this module.
    pub diagnostics: Vec<BindingDiagnostic>,
    /// Non-fatal findings.
    #[napi(js_name = "lint_findings")]
    pub lint_findings: Vec<BindingLintFinding>,
    /// Effective severity per lint rule.
    #[napi(
        js_name = "lint_policy",
        ts_type = "Record<string, 'warn' | 'deny' | 'off'>"
    )]
    pub lint_policy: BTreeMap<String, String>,
}

/// One composition edge: [`Self::derived`] includes the payload of [`Self::base`].
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeStyleDependency {
    /// The style call that consumes another payload.
    pub derived: NativeIdentity,
    /// The style call whose payload is consumed.
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

/// One compiler error, in both byte and editor coordinates.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct BindingDiagnostic {
    /// Stable diagnostic code, such as `SH001`.
    pub code: String,
    /// Rendered message.
    pub message: String,
    /// Path of the file the primary label points at.
    pub file: String,
    /// Byte start of the primary label.
    pub start: u32,
    /// Byte end of the primary label.
    pub end: u32,
    /// UTF-16 offset of the primary label start.
    #[napi(js_name = "start_utf16")]
    pub start_utf16: u32,
    /// UTF-16 offset of the primary label end.
    #[napi(js_name = "end_utf16")]
    pub end_utf16: u32,
    /// Zero-based line of the primary label start.
    #[napi(js_name = "start_line")]
    pub start_line: u32,
    /// Zero-based UTF-16 column of the primary label start.
    #[napi(js_name = "start_column")]
    pub start_column: u32,
    /// Zero-based line of the primary label end.
    #[napi(js_name = "end_line")]
    pub end_line: u32,
    /// Zero-based UTF-16 column of the primary label end.
    #[napi(js_name = "end_column")]
    pub end_column: u32,
    /// Additional explanation lines.
    pub notes: Vec<String>,
}

/// One lint finding.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct BindingLintFinding {
    /// Rule name, as used by `swlint-disable` directives.
    pub rule: String,
    /// Rendered message.
    pub message: String,
    /// Path of the file the finding points at.
    pub file: String,
    /// Byte start of the finding.
    pub start: u32,
    /// Byte end of the finding.
    pub end: u32,
    /// Additional explanation lines.
    pub notes: Vec<String>,
}

/// A url() reference the host must resolve.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeModuleResource {
    /// Placeholder left in the generated CSS.
    pub placeholder: String,
    /// Module that owns the reference.
    pub owner: String,
    /// Authored specifier.
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

/// An applied compiler edit, exposed to JavaScript for source-map construction.
/// Offsets are absolute UTF-16 code units before host prefix insertion or import removal;
/// generated gaps outside `expressions` use the edit's original start as an anchor.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeTransformMapping {
    /// Inclusive original offset; may equal `original_end` for insertion.
    #[napi(js_name = "original_start")]
    pub original_start: u32,
    /// Exclusive original offset, at or after `original_start`.
    #[napi(js_name = "original_end")]
    pub original_end: u32,
    /// Inclusive generated offset in the complete compiled buffer.
    #[napi(js_name = "generated_start")]
    pub generated_start: u32,
    /// Exclusive generated offset; may equal `generated_start` for deletion.
    #[napi(js_name = "generated_end")]
    pub generated_end: u32,
    /// Nonoverlapping intervals contained in this replacement, ordered by generated start.
    /// Original positions may repeat or move backwards; an empty array means anchor-only mapping.
    pub expressions: Vec<NativeExpressionMapping>,
}

impl From<TransformMapping> for NativeTransformMapping {
    /// Copies compiler coordinates and nested intervals without sorting or rebasing them.
    fn from(value: TransformMapping) -> Self {
        Self {
            original_start: value.original_start,
            original_end: value.original_end,
            generated_start: value.generated_start,
            generated_end: value.generated_end,
            expressions: value.expressions.into_iter().map(Into::into).collect(),
        }
    }
}

/// One retained expression or encoded character inside a compiler replacement.
/// Coordinates are absolute UTF-16 code units in the original and complete compiled
/// buffers; exact intervals advance both positions, while encoded intervals use an anchor.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeExpressionMapping {
    /// Inclusive original UTF-16 offset; the anchor when `exact` is false.
    #[napi(js_name = "original_start")]
    pub original_start: u32,
    /// Exclusive original UTF-16 offset of the expression or encoded character.
    #[napi(js_name = "original_end")]
    pub original_end: u32,
    /// Inclusive generated UTF-16 offset within the enclosing replacement.
    #[napi(js_name = "generated_start")]
    pub generated_start: u32,
    /// Exclusive generated UTF-16 offset, strictly greater than `generated_start`.
    #[napi(js_name = "generated_end")]
    pub generated_end: u32,
    /// `true` means identical text and equal interval lengths; `false` maps every
    /// generated code unit to `original_start`, as for an HTML entity.
    pub exact: bool,
}

impl From<ExpressionTransformMapping> for NativeExpressionMapping {
    /// Preserves coordinates and exactness from the validated compiler interval.
    fn from(value: ExpressionTransformMapping) -> Self {
        Self {
            original_start: value.original_start,
            original_end: value.original_end,
            generated_start: value.generated_start,
            generated_end: value.generated_end,
            exact: value.exact,
        }
    }
}

/// A byte span with an explicit file id.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeSpan {
    /// File the span belongs to.
    pub file: u32,
    /// Byte start.
    pub start: u32,
    /// Byte end.
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

/// One conditional test on the path that selects a generated class.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeGuardStep {
    /// The guard expression, which the runtime evaluates as written.
    pub test: NativeSpan,
    /// Whether the class applies when the guard holds.
    #[napi(js_name = "when_true")]
    pub when_true: bool,
}

impl From<GuardStep> for NativeGuardStep {
    fn from(value: GuardStep) -> Self {
        Self {
            test: value.test.into(),
            when_true: value.when_true,
        }
    }
}

/// A class name generated for one utility literal.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct NativeGeneratedClass {
    /// The generated class.
    #[napi(js_name = "class_name")]
    pub class_name: String,
    /// Span of the call the class belongs to.
    #[napi(js_name = "call_span")]
    pub call_span: NativeSpan,
    /// Outermost to innermost conditional tests that select this class.
    #[napi(js_name = "guard_path")]
    pub guard_path: Vec<NativeGuardStep>,
}

impl From<GeneratedClass> for NativeGeneratedClass {
    fn from(value: GeneratedClass) -> Self {
        Self {
            class_name: value.class_name,
            call_span: value.call_span.into(),
            guard_path: value.guard_path.into_iter().map(Into::into).collect(),
        }
    }
}

/// A generated identifier that host tooling must treat as reserved.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct NativeGeneratedSymbol {
    /// The generated name.
    pub name: String,
    /// Kind of declaration that produced it.
    pub kind: String,
    /// Stable identity of the declaration site.
    pub identity: String,
    /// Module the declaration lives in.
    #[napi(js_name = "source_path")]
    pub source_path: String,
    /// Byte start of the declaration.
    pub start: u32,
    /// Byte end of the declaration.
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

/// Output of a whole-graph compile, one entry per requested module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
pub struct ProjectBindingOutput {
    /// Per-module results. Rules shared between modules appear once.
    pub modules: Vec<ProjectModuleOutput>,
}

/// One module's result inside a whole-graph compile.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
pub struct ProjectModuleOutput {
    /// Composition edges of this module.
    pub composition: Vec<NativeStyleDependency>,
    /// Module path.
    pub path: String,
    /// Assets and resources this module references.
    pub resources: Vec<NativeModuleResource>,
    /// Generated CSS. Rules shared with an earlier module are already removed.
    pub css: String,
    /// Completely transformed source, or `None` when compilation failed.
    pub code: Option<String>,
    /// Mappings from the transformed source back to the original.
    pub mappings: Vec<NativeTransformMapping>,
    /// Class names generated for this module's calls.
    pub classes: Vec<NativeGeneratedClass>,
    /// Generated identifiers and their identities.
    pub symbols: Vec<NativeGeneratedSymbol>,
    /// Errors. Non-empty means nothing was published for this module.
    pub diagnostics: Vec<BindingDiagnostic>,
    /// Non-fatal findings.
    #[napi(js_name = "lint_findings")]
    pub lint_findings: Vec<BindingLintFinding>,
    /// Effective severity per lint rule.
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
