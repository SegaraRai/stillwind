//! Producer diagnostics whose labels retain the source revisions they describe.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_source::{SourceRegistry, SourceRegistryError};

use crate::{
    CompileDiagnostic, CompileError, DiagnosticInfo, FixAction, LabelScope, LintDiagnostic,
    LintFinding, SourceRange,
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

    /// Resolves every label of an internal compilation error through its source registry.
    ///
    /// # Errors
    ///
    /// Returns an error if a label names an unregistered or invalid source range.
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

    /// Resolves every label of an internal lint finding through its source registry.
    ///
    /// # Errors
    ///
    /// Returns an error if a label names an unregistered or invalid source range.
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
        diagnostic.related = info.labels[1..]
            .iter()
            .map(|label| {
                Ok(LocatedLabel {
                    scope: label.scope,
                    range: SourceRange::from_span(sources, label.span)?,
                    message: label.message.clone(),
                })
            })
            .collect::<Result<Vec<_>, SourceRegistryError>>()?;
        diagnostic.notes.clone_from(&info.notes);
        diagnostic.actions.clone_from(&info.actions);
        Ok(diagnostic)
    }
}
