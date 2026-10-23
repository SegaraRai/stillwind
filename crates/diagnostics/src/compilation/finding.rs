//! Source highlights and notes attached to fatal diagnostics.

use serde::Serialize;

use stillwind_source::{SourceFile, Span};

use super::{CompileDiagnostic, CompileErrorCode};
use crate::{DiagnosticInfo, Label};

/// One fatal error with its source highlights and supplementary context.
///
/// [`Self::new`] attaches one primary label; callers may add highlights and notes.
/// Display contains the diagnostic message. Hosts format source locations, labels,
/// and notes separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("{diagnostic}")]
#[serde(rename_all = "camelCase")]
pub struct CompileError {
    /// Typed cause; owns the message template and reporting code.
    #[source]
    pub diagnostic: CompileDiagnostic,
    /// Mandatory primary location and shared diagnostic details.
    pub info: Box<DiagnosticInfo>,
}

impl CompileError {
    /// Attaches a document label when no precise original range is available.
    pub fn for_document(diagnostic: CompileDiagnostic, source: &SourceFile) -> Self {
        let mut error = Self::new(diagnostic, source.full_span());
        error.labels[0] = Label::document(source);
        error
    }

    /// Attaches `span` as the sole primary label, with no source name or notes.
    /// The caller supplies a valid reporting span; rejected coordinates remain in
    /// `diagnostic`.
    pub fn new(diagnostic: CompileDiagnostic, span: Span) -> Self {
        Self {
            diagnostic,
            info: Box::new(DiagnosticInfo::new(span)),
        }
    }

    /// Returns the cause's stable reporting code.
    #[must_use]
    pub const fn code(&self) -> CompileErrorCode {
        self.diagnostic.code()
    }
}

impl std::ops::Deref for CompileError {
    type Target = DiagnosticInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl std::ops::DerefMut for CompileError {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use stillwind_source::{FileId, Span};

    use super::*;
    use crate::{Label, LabelScope, LintDiagnostic, LintFinding};

    #[test]
    fn distinct_causes_have_distinct_codes_and_messages() {
        let conflict = CompileDiagnostic::FrameworkConstraintConflict;
        let cycle = CompileDiagnostic::CompositionCycle;
        let parse = CompileDiagnostic::JavaScriptParse;
        assert_ne!(conflict, cycle);
        assert_eq!(
            conflict.code(),
            CompileErrorCode::FrameworkConstraintConflict
        );
        assert_ne!(cycle.code(), conflict.code());
        assert_ne!(parse.code(), conflict.code());
        assert_eq!(
            conflict.to_lean_string(),
            "explicit framework conflicts with the source component format"
        );
        assert_eq!(cycle.to_lean_string(), "style composition dependency cycle");
        assert_eq!(parse.to_lean_string(), "JavaScript parse error");
    }

    #[test]
    fn wrappers_serialize_owned_causes_labels_and_notes() {
        let span = Span::new(FileId(7), 4, 12);
        let mut error = CompileError::new(
            CompileDiagnostic::GeneratedIdentifierCollision {
                name: "衝突".into(),
            },
            span,
        );
        error.labels.push(Label {
            scope: LabelScope::Range,
            span: Span::new(FileId(8), 0, 3),
            message: Some("previous definition".into()),
        });
        error
            .notes
            .push("reported while publishing the module".into());
        let json = serde_json::to_string(&error).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["info"]["labels"].as_array().unwrap().len(), 2);
        assert_eq!(
            value["info"]["notes"][0],
            "reported while publishing the module"
        );
        assert_eq!(error.code(), CompileErrorCode::GeneratedIdentifierCollision);
        assert_eq!(
            value["diagnostic"]["generatedIdentifierCollision"]["name"],
            "衝突"
        );
        assert!(value.get("kind").is_none());
        assert_eq!(
            error.to_lean_string(),
            "generated identifier collision: 衝突"
        );
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_lean_string(),
            error.to_lean_string()
        );

        let finding = LintFinding::new(
            LintDiagnostic::MissingRefAttachment {
                name: "対象".into(),
            },
            span,
        );
        let json = serde_json::to_string(&finding).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["info"]["labels"].as_array().unwrap().len(), 1);
        assert_eq!(finding.code(), "missing-ref-attachment");
        assert_eq!(value["diagnostic"]["missingRefAttachment"]["name"], "対象");
        assert!(value.get("kind").is_none());
        assert_eq!(
            finding.to_lean_string(),
            "ref `対象` provider has no matching marker attachment in this style() call"
        );
    }

    #[test]
    fn generator_failures_report_their_family_and_message() {
        let span = Span::new(FileId(7), 4, 12);
        let error = CompileError::new(
            CompileDiagnostic::IdentifierGenerationFailed {
                family: "keyframes".into(),
                detail: "no name for this declaration".into(),
            },
            span,
        );
        assert_eq!(error.code(), CompileErrorCode::IdentifierGenerationFailed);
        assert_eq!(
            error.to_lean_string(),
            "identifier generation failed for keyframes: no name for this declaration"
        );
        let json = serde_json::to_string(&error).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["info"]["labels"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn internal_error_display_contains_only_the_typed_cause() {
        let error = CompileError::new(
            CompileDiagnostic::UnterminatedCssBlock { offset: 8 },
            Span::new(FileId(77), 0, 9),
        );
        assert_eq!(error.to_lean_string(), error.diagnostic.to_lean_string());
        assert_eq!(error.code(), error.diagnostic.code());
        let value: serde_json::Value = serde_json::to_value(&error).unwrap();
        assert!(value["info"].get("source_name").is_none());
    }
}
