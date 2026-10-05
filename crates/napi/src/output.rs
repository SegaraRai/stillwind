//! Serializable compiler output.
//!
//! [`BindingOutput`] carries module results from Rust services to JavaScript adapters.
//! [`BindingReport`] carries the common reporting layer's document identity, revision,
//! complete editor range, related highlights, and fix actions. The compatibility
//! diagnostic and lint records carry primary coordinates too. Other records document
//! their own coordinate units at the field level.
//!
//! Generated code is absent when compiler diagnostics are present. These types describe
//! one result; retaining an earlier successful result is the host session's responsibility.

use std::collections::BTreeMap;

use napi_derive::napi;
use serde::Serialize;

use stillwind_compiler::{
    ExpressionTransformMapping, GeneratedClass, GeneratedSymbol, GuardStep,
    ModuleResourceReference, RejectedIdentifier, SymbolOrigin, TransformMapping,
};
use stillwind_diagnostics::{
    DiagnosticBatch, DiagnosticSeverity, LabelScope, ResolvedAction, ResolvedDiagnostic,
    ResolvedEdit, ResolvedLabel, ResolvedLocation,
};
use stillwind_ir::StyleDependency;
use stillwind_source::{ModuleId, Span};

use crate::semantic::{NativeIdentity, NativeModuleSummary};

/// Result of compiling one module.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// Source-resolved diagnostics, including related locations and fixes.
    pub reports: Vec<BindingReport>,
    /// Names a host-generated identifier was rejected for, which the host may warn about.
    ///
    /// These are not lint findings: a rejected name comes from the generator rather than
    /// from the source, so the entry carries the family, the request hash, and the reason
    /// instead of a source span.
    pub rejected_names: Vec<BindingRejectedIdentifier>,
    /// Non-fatal findings.
    pub lint_findings: Vec<BindingLintFinding>,
    /// Effective severity per lint rule.
    #[napi(ts_type = "Record<string, 'warn' | 'deny' | 'off'>")]
    pub lint_settings: BTreeMap<String, String>,
}

/// One source-resolved diagnostic transferred from the common reporting layer.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingReport {
    /// Stable compiler code or lint rule.
    pub code: String,
    /// Human-readable cause.
    pub message: String,
    /// Effective `error` or `warning` level after lint policy.
    pub severity: String,
    /// Main highlighted range.
    pub primary: BindingLabel,
    /// Further highlighted ranges in producer order.
    pub related: Vec<BindingLabel>,
    /// Supplementary explanations.
    pub notes: Vec<String>,
    /// Alternative revision-pinned fixes.
    pub actions: Vec<BindingAction>,
}

impl From<ResolvedDiagnostic> for BindingReport {
    fn from(value: ResolvedDiagnostic) -> Self {
        Self {
            code: value.code,
            message: value.message,
            severity: match value.severity {
                DiagnosticSeverity::Error => "error",
                DiagnosticSeverity::Warning => "warning",
            }
            .into(),
            primary: value.primary.into(),
            related: value.related.into_iter().map(Into::into).collect(),
            notes: value.notes,
            actions: value.actions.into_iter().map(Into::into).collect(),
        }
    }
}

/// A highlight and its explanation.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingLabel {
    /// Whether the producer identified a range or only the document.
    #[napi(ts_type = "'range' | 'document'")]
    pub scope: String,
    /// Source revision and editor range.
    pub location: BindingLocation,
    /// Explanation for this range, when one was supplied.
    pub message: Option<String>,
}

impl From<ResolvedLabel> for BindingLabel {
    fn from(value: ResolvedLabel) -> Self {
        Self {
            scope: match value.scope {
                LabelScope::Range => "range",
                LabelScope::Document => "document",
            }
            .into(),
            location: value.location.into(),
            message: value.message,
        }
    }
}

/// Document identity, revision, and complete half-open editor range.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingLocation {
    /// Absolute document URI supplied by the host or formed for a virtual input.
    pub uri: String,
    /// Fingerprint of the exact source text used for these coordinates.
    pub revision: String,
    /// Inclusive UTF-8 byte offset.
    pub start: u32,
    /// Exclusive UTF-8 byte offset.
    pub end: u32,
    /// Zero-based UTF-16 offset from the document start.
    pub start_utf16: u32,
    /// Zero-based UTF-16 offset from the document start.
    pub end_utf16: u32,
    /// Zero-based start line.
    pub start_line: u32,
    /// Zero-based UTF-16 column on `start_line`.
    pub start_column: u32,
    /// Zero-based end line.
    pub end_line: u32,
    /// Zero-based UTF-16 column on `end_line`.
    pub end_column: u32,
}

impl From<ResolvedLocation> for BindingLocation {
    fn from(value: ResolvedLocation) -> Self {
        Self {
            uri: value.uri.as_str().into(),
            revision: value.revision,
            start: value.start,
            end: value.end,
            start_utf16: value.start_utf16,
            end_utf16: value.end_utf16,
            start_line: value.start_position.line,
            start_column: value.start_position.utf16_column,
            end_line: value.end_position.line,
            end_column: value.end_position.utf16_column,
        }
    }
}

/// A revision-pinned replacement edit.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingEdit {
    /// Range in the source revision to replace.
    pub location: BindingLocation,
    /// Text inserted at the range.
    pub replacement: String,
}

impl From<ResolvedEdit> for BindingEdit {
    fn from(value: ResolvedEdit) -> Self {
        Self {
            location: value.location.into(),
            replacement: value.replacement,
        }
    }
}

/// One alternative fix offered for a diagnostic.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingAction {
    /// Title shown by an editor.
    pub title: String,
    /// Edits applied atomically after verifying every source revision.
    pub edits: Vec<BindingEdit>,
}

impl From<ResolvedAction> for BindingAction {
    fn from(value: ResolvedAction) -> Self {
        Self {
            title: value.title,
            edits: value.edits.into_iter().map(Into::into).collect(),
        }
    }
}

/// One composition edge: [`Self::derived`] includes the payload of [`Self::base`].
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct BindingDiagnostic {
    /// Whether the producer identified a range or only the document.
    #[napi(ts_type = "'range' | 'document'")]
    pub scope: String,
    /// Stable diagnostic code, such as `SW02003`.
    pub code: String,
    /// Rendered message.
    pub message: String,
    /// Exact source document identity.
    pub uri: String,
    /// Fingerprint of the source revision used for the range.
    pub revision: String,
    /// Byte start of the primary label.
    pub start: u32,
    /// Byte end of the primary label.
    pub end: u32,
    /// UTF-16 offset of the primary label start.
    pub start_utf16: u32,
    /// UTF-16 offset of the primary label end.
    pub end_utf16: u32,
    /// Zero-based line of the primary label start.
    pub start_line: u32,
    /// Zero-based UTF-16 column of the primary label start.
    pub start_column: u32,
    /// Zero-based line of the primary label end.
    pub end_line: u32,
    /// Zero-based UTF-16 column of the primary label end.
    pub end_column: u32,
    /// Additional explanation lines.
    pub notes: Vec<String>,
}

/// One lint finding.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingLintFinding {
    /// Rule name, as used by `swlint-disable` directives.
    pub rule: String,
    /// Rendered message.
    pub message: String,
    /// Exact source document identity.
    pub uri: String,
    /// Fingerprint of the source revision used for the range.
    pub revision: String,
    /// Byte start of the finding.
    pub start: u32,
    /// Byte end of the finding.
    pub end: u32,
    /// Zero-based line of the primary range start.
    pub start_line: u32,
    /// Zero-based UTF-16 column of the primary range start.
    pub start_column: u32,
    /// Zero-based line of the primary range end.
    pub end_line: u32,
    /// Zero-based UTF-16 column of the primary range end.
    pub end_column: u32,
    /// Additional explanation lines.
    pub notes: Vec<String>,
}

/// A `url()` reference the host must resolve.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeModuleResource {
    /// Placeholder left in the generated CSS.
    pub placeholder: String,
    /// Module that owns the reference.
    pub module_id: String,
    /// Authored specifier.
    pub specifier: String,
}

impl From<ModuleResourceReference> for NativeModuleResource {
    fn from(value: ModuleResourceReference) -> Self {
        Self {
            placeholder: value.placeholder,
            module_id: value.owner.into_string(),
            specifier: value.specifier,
        }
    }
}

/// An applied compiler edit, exposed to JavaScript for source-map construction.
/// Offsets are absolute UTF-16 code units before host prefix insertion or import removal;
/// generated gaps outside `expressions` use the edit's original start as an anchor.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTransformMapping {
    /// Inclusive original offset; may equal `original_end` for insertion.
    pub original_start: u32,
    /// Exclusive original offset, at or after `original_start`.
    pub original_end: u32,
    /// Inclusive generated offset in the complete compiled buffer.
    pub generated_start: u32,
    /// Exclusive generated offset; may equal `generated_start` for deletion.
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
#[serde(rename_all = "camelCase")]
pub struct NativeExpressionMapping {
    /// Inclusive original UTF-16 offset; the anchor when `exact` is false.
    pub original_start: u32,
    /// Exclusive original UTF-16 offset of the expression or encoded character.
    pub original_end: u32,
    /// Inclusive generated UTF-16 offset within the enclosing replacement.
    pub generated_start: u32,
    /// Exclusive generated UTF-16 offset, strictly greater than `generated_start`.
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
#[serde(rename_all = "camelCase")]
pub struct NativeSpan {
    /// File ID within the session's source scope.
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
#[serde(rename_all = "camelCase")]
pub struct NativeGuardStep {
    /// The guard expression, which the runtime evaluates as written.
    pub test: NativeSpan,
    /// Whether the class applies when the guard holds.
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
#[serde(rename_all = "camelCase")]
pub struct NativeGeneratedClass {
    /// The generated class.
    pub class_name: String,
    /// Span of the call the class belongs to.
    pub call_span: NativeSpan,
    /// Outermost to innermost conditional tests that select this class.
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
#[serde(rename_all = "camelCase")]
pub struct NativeGeneratedSymbol {
    /// The generated name.
    pub name: String,
    /// Kind of declaration that produced it.
    pub kind: String,
    /// Owner identity used to distinguish name reuse from a collision.
    pub identity: String,
    /// Domain of the source that owns this name: `module` or `stylesheet`.
    pub source_kind: String,
    /// Identity within `source_kind`.
    pub source_id: String,
    /// Byte start of the declaration, absent for a name that declares nothing itself.
    pub start: Option<u32>,
    /// Byte end of the declaration, absent for a name that declares nothing itself.
    pub end: Option<u32>,
}

impl From<GeneratedSymbol> for NativeGeneratedSymbol {
    fn from(value: GeneratedSymbol) -> Self {
        Self {
            name: value.name,
            kind: value.kind,
            identity: value.identity,
            source_kind: match &value.origin {
                SymbolOrigin::Module(_) => "module",
                SymbolOrigin::Stylesheet(_) => "stylesheet",
            }
            .to_owned(),
            source_id: match value.origin {
                SymbolOrigin::Module(id) => id.into_string(),
                SymbolOrigin::Stylesheet(id) => id.as_str().to_owned(),
            },
            start: value.start,
            end: value.end,
        }
    }
}

/// One rejected generated name, reported for a host that supplies its own generators.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingRejectedIdentifier {
    /// Identifier family the generator was asked for.
    pub kind: String,
    /// Canonical hash of the request, which identifies the rejected input.
    pub hash: String,
    /// Why the name was rejected.
    pub reason: String,
    /// Whether the built-in name was substituted for it.
    pub substituted: bool,
}

impl From<RejectedIdentifier> for BindingRejectedIdentifier {
    fn from(value: RejectedIdentifier) -> Self {
        Self {
            kind: value.kind.as_str().to_owned(),
            hash: value.hash,
            reason: value.reason,
            substituted: value.substituted,
        }
    }
}

/// Output of a whole-graph compile, one entry per requested module.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBindingOutput {
    /// Per-module results. Rules shared between modules appear once.
    pub modules: Vec<ProjectModuleOutput>,
}

/// Latest source-resolved findings for one module's attempted revision.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDiagnosticBatch {
    /// Project module identity for the attempted analysis.
    pub module: String,
    /// Diagnostics in report order.
    pub reports: Vec<BindingReport>,
}

impl NativeDiagnosticBatch {
    /// Copies a resolved batch for the host while the session retains its own batch.
    pub(crate) fn new(module: &ModuleId, batch: &DiagnosticBatch) -> Self {
        Self {
            module: module.as_str().into(),
            reports: batch.diagnostics.iter().cloned().map(Into::into).collect(),
        }
    }
}

/// Scan outcome; only a verified scan supplies a summary for checker queries.
// NAPI object conversion does not accept a boxed summary.
#[allow(clippy::large_enum_variant)]
#[napi(
    discriminant = "kind",
    discriminant_case = "lowercase",
    object_from_js = false
)]
pub enum NativeScanResult {
    /// Original-coordinate summary from a verified source.
    Ready {
        /// Summary to use for dependency traversal and checker queries.
        summary: NativeModuleSummary,
    },
    /// Fatal syntax or projection failures in the original document.
    Failed {
        /// Complete reports for the rejected source revision.
        batch: NativeDiagnosticBatch,
    },
}

/// One module's result inside a whole-graph compile.
#[napi(object, object_from_js = false, use_nullable = true)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModuleOutput {
    /// Composition edges of this module.
    pub composition: Vec<NativeStyleDependency>,
    /// Host-chosen identity of this module.
    pub module_id: String,
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
    /// Source-resolved diagnostics for this module.
    pub reports: Vec<BindingReport>,
    /// Names a host-generated identifier was rejected for, which the host may warn about.
    pub rejected_names: Vec<BindingRejectedIdentifier>,
    /// Non-fatal findings.
    pub lint_findings: Vec<BindingLintFinding>,
    /// Effective severity per lint rule.
    #[napi(ts_type = "Record<string, 'warn' | 'deny' | 'off'>")]
    pub lint_settings: BTreeMap<String, String>,
}

impl ProjectModuleOutput {
    pub(crate) fn new(module_id: ModuleId, output: BindingOutput) -> Self {
        let BindingOutput {
            composition,
            resources,
            css,
            code,
            mappings,
            classes,
            symbols,
            diagnostics,
            reports,
            rejected_names,
            lint_findings,
            lint_settings,
        } = output;
        Self {
            composition,
            module_id: module_id.into_string(),
            resources,
            css,
            code,
            mappings,
            classes,
            symbols,
            diagnostics,
            reports,
            rejected_names,
            lint_findings,
            lint_settings,
        }
    }
}
