//! Transfers resolved diagnostics, editor ranges, fixes, and scan failures.

use napi::bindgen_prelude::{Env, JsObjectValue};
use napi::{Error, Status};
use napi_derive::napi;
use serde::Serialize;

use stillwind_diagnostics::{
    DiagnosticBatch, DiagnosticSeverity, LabelScope, ResolvedAction, ResolvedDiagnostic,
    ResolvedEdit, ResolvedLabel, ResolvedLocation,
};
use stillwind_source::ModuleId;

use super::NativeText;
use crate::semantic::NativeModuleSummary;

/// Throws an error with source-resolved diagnostics attached as `reports`.
pub(crate) fn report_error(env: Env, message: String, reports: Vec<BindingReport>) -> Error {
    let create = || -> napi::Result<Error> {
        let mut value = env.create_error(Error::from_reason(message))?;
        value.set_named_property("reports", reports)?;
        env.throw(value)?;
        Ok(Error::new(Status::PendingException, "Analysis failed"))
    };
    create().unwrap_or_else(|error| error)
}

/// One source-resolved diagnostic transferred from the common reporting layer.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingReport {
    /// Stable compiler code or lint rule.
    #[napi(ts_type = "string")]
    pub code: NativeText,
    /// Human-readable cause.
    #[napi(ts_type = "string")]
    pub message: NativeText,
    /// Effective `error` or `warning` level after lint policy.
    #[napi(ts_type = "string")]
    pub severity: NativeText,
    /// Main highlighted range.
    pub primary: BindingLabel,
    /// Further highlighted ranges in producer order.
    pub related: Vec<BindingLabel>,
    /// Supplementary explanations.
    #[napi(ts_type = "Array<string>")]
    pub notes: Vec<NativeText>,
    /// Alternative revision-pinned fixes.
    pub actions: Vec<BindingAction>,
}

impl From<ResolvedDiagnostic> for BindingReport {
    fn from(value: ResolvedDiagnostic) -> Self {
        Self {
            code: (value.code).into(),
            message: (value.message).into(),
            severity: match value.severity {
                DiagnosticSeverity::Error => "error",
                DiagnosticSeverity::Warning => "warning",
            }
            .into(),
            primary: value.primary.into(),
            related: value.related.into_iter().map(Into::into).collect(),
            notes: (value.notes).into_iter().map(Into::into).collect(),
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
    pub scope: NativeText,
    /// Source revision and editor range.
    pub location: BindingLocation,
    /// Explanation for this range, when one was supplied.
    #[napi(ts_type = "string")]
    pub message: Option<NativeText>,
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
            message: (value.message).map(Into::into),
        }
    }
}

/// Document identity, revision, and complete half-open editor range.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingLocation {
    /// Absolute document URI supplied by the host or formed for a virtual input.
    #[napi(ts_type = "string")]
    pub uri: NativeText,
    /// Fingerprint of the exact source text used for these coordinates.
    #[napi(ts_type = "string")]
    pub revision: NativeText,
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
            uri: value.uri.into(),
            revision: (value.revision).into(),
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
    #[napi(ts_type = "string")]
    pub replacement: NativeText,
}

impl From<ResolvedEdit> for BindingEdit {
    fn from(value: ResolvedEdit) -> Self {
        Self {
            location: value.location.into(),
            replacement: (value.replacement).into(),
        }
    }
}

/// One alternative fix offered for a diagnostic.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingAction {
    /// Title shown by an editor.
    #[napi(ts_type = "string")]
    pub title: NativeText,
    /// Edits applied atomically after verifying every source revision.
    pub edits: Vec<BindingEdit>,
}

impl From<ResolvedAction> for BindingAction {
    fn from(value: ResolvedAction) -> Self {
        Self {
            title: (value.title).into(),
            edits: value.edits.into_iter().map(Into::into).collect(),
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
    pub scope: NativeText,
    /// Stable diagnostic code, such as `SW01012`.
    #[napi(ts_type = "string")]
    pub code: NativeText,
    /// Rendered message.
    #[napi(ts_type = "string")]
    pub message: NativeText,
    /// Exact source document identity.
    #[napi(ts_type = "string")]
    pub uri: NativeText,
    /// Fingerprint of the source revision used for the range.
    #[napi(ts_type = "string")]
    pub revision: NativeText,
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
    #[napi(ts_type = "Array<string>")]
    pub notes: Vec<NativeText>,
}

/// One lint finding.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingLintFinding {
    /// Rule name, as used by `swlint-disable` directives.
    #[napi(ts_type = "string")]
    pub rule: NativeText,
    /// Rendered message.
    #[napi(ts_type = "string")]
    pub message: NativeText,
    /// Exact source document identity.
    #[napi(ts_type = "string")]
    pub uri: NativeText,
    /// Fingerprint of the source revision used for the range.
    #[napi(ts_type = "string")]
    pub revision: NativeText,
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
    #[napi(ts_type = "Array<string>")]
    pub notes: Vec<NativeText>,
}

/// Latest source-resolved findings for one module's attempted revision.
#[napi(object, object_from_js = false)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDiagnosticBatch {
    /// Project module identity for the attempted analysis.
    #[napi(ts_type = "string")]
    pub module: NativeText,
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

    /// Moves a completed batch into host output when no project state retains it.
    pub(crate) fn from_owned(module: &ModuleId, batch: DiagnosticBatch) -> Self {
        Self {
            module: module.as_str().into(),
            reports: batch.diagnostics.into_iter().map(Into::into).collect(),
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
