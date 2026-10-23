//! Producer diagnostics whose labels retain the source revisions they describe.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_source::{SourceRegistry, SourceRegistryError};

use crate::{
    CompileDiagnostic, CompileError, CompileErrorClass, DiagnosticInfo, FixAction, LabelScope,
    LintDiagnostic, LintFinding, SourceRange,
};

/// Typed cause of a completed, source-located public diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticCause {
    /// Fatal compilation error.
    Compile(CompileDiagnostic),
    /// Suppressible lint finding.
    Lint(LintDiagnostic),
}

impl DiagnosticCause {
    pub(super) fn code(&self) -> &'static str {
        match self {
            Self::Compile(diagnostic) => diagnostic.code().as_str(),
            Self::Lint(kind) => kind.code(),
        }
    }

    pub(super) const fn class(&self) -> Option<CompileErrorClass> {
        match self {
            Self::Compile(diagnostic) => Some(diagnostic.code().class()),
            Self::Lint(_) => None,
        }
    }

    pub(super) fn message(&self) -> LeanString {
        match self {
            Self::Compile(kind) => kind.to_lean_string(),
            Self::Lint(kind) => kind.to_lean_string(),
        }
    }
}

/// One highlight tied to the exact source revision used for its coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatedLabel {
    /// Whether the producer identified a range or only the document.
    pub scope: LabelScope,
    /// Validated source revision and byte range.
    pub range: SourceRange,
    /// Explanation shown beside the highlight.
    pub message: Option<LeanStr>,
}

/// A completed diagnostic with source-bound labels and optional corrections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatedDiagnostic {
    /// Cause controlling the reporting code and severity policy.
    pub cause: DiagnosticCause,
    /// Mandatory main highlight.
    pub primary: LocatedLabel,
    /// Further highlights in producer order.
    pub related: Vec<LocatedLabel>,
    /// Supplementary explanations.
    pub notes: Vec<LeanStr>,
    /// Alternative user-invoked corrections.
    pub actions: Vec<FixAction>,
}

impl LocatedDiagnostic {
    /// Starts a diagnostic with a source-bound primary location.
    pub fn new(cause: DiagnosticCause, primary: SourceRange) -> Self {
        Self {
            cause,
            primary: LocatedLabel {
                scope: LabelScope::Range,
                range: primary,
                message: None,
            },
            related: Vec::new(),
            notes: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Resolves a compilation error through its source registry, omitting related
    /// locations in internal buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if a label names an unregistered or invalid source range, or if
    /// the primary label refers to an internal buffer.
    pub fn from_compile(
        error: &CompileError,
        sources: &SourceRegistry,
    ) -> Result<Self, SourceRegistryError> {
        Self::from_internal(
            DiagnosticCause::Compile(error.diagnostic.clone()),
            &error.info,
            sources,
        )
    }

    /// Resolves a lint finding through its source registry, omitting related locations
    /// in internal buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if a label names an unregistered or invalid source range, or if
    /// the primary label refers to an internal buffer.
    pub fn from_lint(
        finding: &LintFinding,
        sources: &SourceRegistry,
    ) -> Result<Self, SourceRegistryError> {
        Self::from_internal(
            DiagnosticCause::Lint(finding.diagnostic.clone()),
            &finding.info,
            sources,
        )
    }

    fn from_internal(
        cause: DiagnosticCause,
        info: &DiagnosticInfo,
        sources: &SourceRegistry,
    ) -> Result<Self, SourceRegistryError> {
        let primary = &info.labels[0];
        let mut diagnostic = Self::new(cause, SourceRange::from_span(sources, primary.span)?);
        diagnostic.primary.message.clone_from(&primary.message);
        diagnostic.primary.scope = primary.scope;
        for label in &info.labels[1..] {
            let range = match SourceRange::from_span(sources, label.span) {
                Ok(range) => range,
                Err(SourceRegistryError::UnreportableSource(_)) => continue,
                Err(error) => return Err(error),
            };
            diagnostic.related.push(LocatedLabel {
                scope: label.scope,
                range,
                message: label.message.clone(),
            });
        }
        diagnostic.notes.clone_from(&info.notes);
        diagnostic.actions.clone_from(&info.actions);
        Ok(diagnostic)
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::{FileId, SourceError, SourceUri, Span};

    use super::*;
    use crate::Label;

    #[test]
    fn internal_related_labels_are_omitted_without_losing_authored_labels() {
        let mut sources = SourceRegistry::default();
        let authored = sources
            .register(
                Some(SourceUri::parse("untitled:main").unwrap()),
                None,
                "abc",
            )
            .unwrap();
        let generated = sources.register(None, None, "generated").unwrap();
        let mut error = CompileError::new(CompileDiagnostic::JavaScriptParse, authored.full_span());
        for (source, message) in [(&generated, "internal"), (&authored, "authored")] {
            error.labels.push(Label {
                scope: LabelScope::Range,
                span: source.full_span(),
                message: Some(message.into()),
            });
        }
        let located = LocatedDiagnostic::from_compile(&error, &sources).unwrap();
        assert_eq!(located.related.len(), 1);
        assert_eq!(located.related[0].message.as_deref(), Some("authored"));
        assert_eq!(located.related[0].range.source().id(), authored.id());
    }

    #[test]
    fn invalid_related_spans_remain_errors_for_internal_buffers() {
        let mut sources = SourceRegistry::default();
        let authored = sources
            .register(
                Some(SourceUri::parse("untitled:main").unwrap()),
                None,
                "abc",
            )
            .unwrap();
        let generated = sources.register(None, None, "😀").unwrap();
        for span in [
            Span::new(FileId(u32::MAX), 0, 1),
            Span::new(generated.id(), 0, 1),
        ] {
            let mut error =
                CompileError::new(CompileDiagnostic::JavaScriptParse, authored.full_span());
            error.labels.push(Label {
                scope: LabelScope::Range,
                span,
                message: None,
            });
            let result = LocatedDiagnostic::from_compile(&error, &sources);
            if span.file == generated.id() {
                assert!(
                    matches!(result, Err(SourceRegistryError::Source(SourceError::InvalidSpan(invalid))) if invalid == span)
                );
            } else {
                assert!(
                    matches!(result, Err(SourceRegistryError::UnknownFile(file)) if file == span.file)
                );
            }
        }
    }
}
