//! Host diagnostics with validated byte and UTF-16 locations in pinned revisions.

use std::fmt::Write;

use lean_string::{LeanStr, LeanString, ToLeanString};
use serde::{Deserialize, Serialize};

use stillwind_source::{Position, SourceUri};

use crate::{CompileErrorClass, LabelScope};

/// Severity after lint policy is applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    /// A compilation failure or denied lint.
    Error,
    /// An advisory lint.
    Warning,
}

/// A validated location in one original source revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLocation {
    /// Host-supplied document identity.
    pub uri: SourceUri,
    /// Fingerprint of the exact source bytes used to compute the positions.
    pub revision: LeanStr,
    /// Inclusive UTF-8 byte start.
    pub start: u32,
    /// Exclusive UTF-8 byte end.
    pub end: u32,
    /// Zero-based UTF-16 offset of the start within the document.
    pub start_utf16: u32,
    /// Zero-based UTF-16 offset of the end within the document.
    pub end_utf16: u32,
    /// Zero-based line and UTF-16 column of the start.
    pub start_position: Position,
    /// Zero-based line and UTF-16 column of the end.
    pub end_position: Position,
}

/// One related highlight, possibly in another document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLabel {
    /// Whether the producer identified a range or only the document.
    #[serde(default)]
    pub scope: LabelScope,
    /// Source and coordinates of this highlight.
    pub location: ResolvedLocation,
    /// Explanation displayed next to the highlight.
    pub message: Option<LeanStr>,
}

/// One proposed replacement within a revision-pinned source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedEdit {
    /// Document and range to replace.
    pub location: ResolvedLocation,
    /// Replacement text.
    pub replacement: LeanStr,
}

/// One alternative user-invoked fix consisting of one or more edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAction {
    /// Title shown by an editor.
    pub title: LeanStr,
    /// Edits applied as one action.
    pub edits: Vec<ResolvedEdit>,
}

/// One complete public diagnostic with its source locations and optional fixes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDiagnostic {
    /// Stable reporting code or lint rule name.
    pub code: LeanStr,
    /// Party that must act on a compilation error. Lint findings have no class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<CompileErrorClass>,
    /// Human-readable cause.
    pub message: LeanStr,
    /// Effective reporting level.
    pub severity: DiagnosticSeverity,
    /// Main location reported for this failure.
    pub primary: ResolvedLabel,
    /// Additional locations, in producer order.
    pub related: Vec<ResolvedLabel>,
    /// Supplementary explanations.
    pub notes: Vec<LeanStr>,
    /// Alternative fixes offered to the user.
    pub actions: Vec<ResolvedAction>,
}

impl ResolvedDiagnostic {
    /// Renders a diagnostic for text-only hosts. Range labels use one-based line and
    /// column numbers. Document labels omit those coordinates.
    pub fn display_text(&self) -> LeanString {
        let at = &self.primary.location;
        let mut text = if self.primary.scope == LabelScope::Document {
            format_args!("{}: {}: {}", at.uri.as_str(), self.code, self.message).to_lean_string()
        } else {
            format_args!(
                "{}:{}:{}-{}:{}: {}: {}",
                at.uri.as_str(),
                at.start_position.line + 1,
                at.start_position.utf16_column + 1,
                at.end_position.line + 1,
                at.end_position.utf16_column + 1,
                self.code,
                self.message,
            )
            .to_lean_string()
        };
        for label in &self.related {
            let at = &label.location;
            if label.scope == LabelScope::Document {
                let _ = write!(text, "\n  {}", at.uri.as_str());
            } else {
                let _ = write!(
                    text,
                    "\n  {}:{}:{}-{}:{}",
                    at.uri.as_str(),
                    at.start_position.line + 1,
                    at.start_position.utf16_column + 1,
                    at.end_position.line + 1,
                    at.end_position.utf16_column + 1
                );
            }
            if let Some(message) = &label.message {
                text.push_str(": ");
                text.push_str(message);
            }
        }
        for note in &self.notes {
            text.push_str("\n  ");
            text.push_str(note);
        }
        text
    }
}

/// Completed diagnostics from one reporting invocation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBatch {
    /// Diagnostics in report order, after suppression and severity policy.
    pub diagnostics: Vec<ResolvedDiagnostic>,
}
