//! Host diagnostics with validated byte and UTF-16 locations in pinned revisions.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use stillwind_source::{Position, SourceUri};

use super::ReportError;
use crate::LabelScope;

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
    pub revision: String,
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
    pub message: Option<String>,
}

/// One proposed replacement within a revision-pinned source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedEdit {
    /// Document and range to replace.
    pub location: ResolvedLocation,
    /// Replacement text.
    pub replacement: String,
}

/// One alternative user-invoked fix consisting of one or more edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAction {
    /// Title shown by an editor.
    pub title: String,
    /// Edits applied as one action.
    pub edits: Vec<ResolvedEdit>,
}

impl ResolvedAction {
    /// Rejects application when any target document has changed since reporting.
    ///
    /// # Errors
    ///
    /// Returns a stale-edit error when the current revision differs or is unavailable.
    pub fn verify_current(
        &self,
        mut current_revision: impl FnMut(&SourceUri) -> Option<String>,
    ) -> Result<(), ReportError> {
        for edit in &self.edits {
            let location = &edit.location;
            if current_revision(&location.uri).as_deref() != Some(&location.revision) {
                return Err(ReportError::StaleEdit(location.uri.as_str().to_owned()));
            }
        }
        Ok(())
    }
}

/// One complete public diagnostic with its source locations and optional fixes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDiagnostic {
    /// Stable reporting code or lint rule name.
    pub code: String,
    /// Human-readable cause.
    pub message: String,
    /// Effective reporting level.
    pub severity: DiagnosticSeverity,
    /// Main location reported for this failure.
    pub primary: ResolvedLabel,
    /// Additional locations, in producer order.
    pub related: Vec<ResolvedLabel>,
    /// Supplementary explanations.
    pub notes: Vec<String>,
    /// Alternative fixes offered to the user.
    pub actions: Vec<ResolvedAction>,
}

impl ResolvedDiagnostic {
    /// Renders a diagnostic for text-only hosts. Range labels use one-based line and
    /// column numbers. Document labels omit those coordinates.
    pub fn display_text(&self) -> String {
        let at = &self.primary.location;
        let mut text = if self.primary.scope == LabelScope::Document {
            format!("{}: {}: {}", at.uri.as_str(), self.code, self.message)
        } else {
            format!(
                "{}:{}:{}-{}:{}: {}: {}",
                at.uri.as_str(),
                at.start_position.line + 1,
                at.start_position.utf16_column + 1,
                at.end_position.line + 1,
                at.end_position.utf16_column + 1,
                self.code,
                self.message,
            )
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
