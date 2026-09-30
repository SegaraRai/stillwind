//! Source highlights and notes attached to fatal diagnostics.

use serde::Serialize;

use stillwind_source::Span;

use super::{CompileDiagnostic, CompileErrorCode};
use crate::DiagnosticInfo;

/// One fatal error with its source highlights and supplementary context.
///
/// [`Self::new`] attaches one primary label; callers may add highlights and notes.
/// Display contains the diagnostic message. Hosts format source locations, labels,
/// and notes separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("{diagnostic}")]
pub struct CompileError {
    /// Typed cause; owns the message template and reporting code.
    #[source]
    pub diagnostic: CompileDiagnostic,
    /// Mandatory primary location and shared diagnostic details.
    pub info: Box<DiagnosticInfo>,
}

impl CompileError {
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
    use stillwind_source::{FileId, Span};

    use super::*;
    use crate::{Label, LintDiagnostic, LintFinding};

    #[test]
    fn distinct_causes_have_distinct_codes_and_messages() {
        let invalid = CompileDiagnostic::InvalidVariantPrefix;
        let unavailable = CompileDiagnostic::UnavailableVariantPrefix;
        let unidentified = CompileDiagnostic::UnidentifiedRef;
        assert_ne!(invalid, unavailable);
        assert_eq!(invalid.code(), CompileErrorCode::InvalidVariantPrefix);
        assert_ne!(unavailable.code(), invalid.code());
        assert_ne!(unidentified.code(), invalid.code());
        assert_eq!(invalid.to_string(), "invalid variant prefix");
        assert_eq!(
            unavailable.to_string(),
            "unknown or unavailable variant prefix"
        );
        assert_eq!(
            unidentified.to_string(),
            "ref variant has no known ref identity"
        );
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
            value["diagnostic"]["GeneratedIdentifierCollision"]["name"],
            "衝突"
        );
        assert!(value.get("kind").is_none());
        assert_eq!(error.to_string(), "generated identifier collision: 衝突");
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            error.to_string()
        );

        let finding = LintFinding::new(
            LintDiagnostic::MissingAnchor {
                name: "対象".into(),
            },
            span,
        );
        let json = serde_json::to_string(&finding).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["info"]["labels"].as_array().unwrap().len(), 1);
        assert_eq!(finding.code(), "require-anchor-provider");
        assert_eq!(value["diagnostic"]["MissingAnchor"]["name"], "対象");
        assert!(value.get("kind").is_none());
        assert_eq!(
            finding.to_string(),
            "ref `対象` uses position without an asAnchor() provider"
        );
    }

    #[test]
    fn generator_failures_report_their_family_and_message() {
        let span = Span::new(FileId(7), 4, 12);
        let error = CompileError::new(
            CompileDiagnostic::IdentifierGenerationFailed(
                "keyframes".into(),
                "no name for this declaration".into(),
            ),
            span,
        );
        assert_eq!(error.code(), CompileErrorCode::IdentifierGenerationFailed);
        assert_eq!(
            error.to_string(),
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
        assert_eq!(error.to_string(), error.diagnostic.to_string());
        assert_eq!(error.code(), error.diagnostic.code());
        let value: serde_json::Value = serde_json::to_value(&error).unwrap();
        assert!(value["info"].get("source_name").is_none());
    }
}
