//! Transfers byte spans and mappings for generated source expressions.

use napi_derive::napi;
use serde::Serialize;

use stillwind_compiler::{ExpressionTransformMapping, TransformMapping};
use stillwind_source::Span;

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
