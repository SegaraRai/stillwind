//! Advisory causes, suppression rule names, and source-located findings.
//!
//! Construct [`LintFinding`] from a [`LintKind`] and the reported source span.
//! [`UtilityFailure`] distinguishes malformed candidate syntax from unresolved names;
//! detection and host severity policy remain outside this module.

use serde::{Deserialize, Serialize};

use stillwind_source::{ModuleId, Span};

use crate::DiagnosticInfo;

/// Configured reporting level of a lint finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    /// Report a warning.
    Warn,
    /// Report an error without discarding suggested fixes.
    Deny,
    /// Omit the finding and its suggested fixes.
    Off,
}

impl LintSeverity {
    /// Returns the CSS configuration and host-protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Off => "off",
        }
    }

    /// Parses a CSS lint level.
    pub fn from_css(value: &str) -> Option<Self> {
        match value {
            "warn" => Some(Self::Warn),
            "deny" => Some(Self::Deny),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// Typed causes and display templates for advisory lint findings.
///
/// Codes are suppression rule names, not discriminants: distinct causes may share a rule.
/// Producers select the cause; hosts decide its presentation and configured severity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum LintKind {
    /// A macro value expression visibly mutates state; default severity is error.
    #[error(
        "macro arguments must be pure; perform assignments, updates, and deletes before the macro call"
    )]
    MacroArgumentMutation,
    /// A macro expression suspends execution or loads a module; default severity is error.
    #[error(
        "evaluate await, yield, and dynamic imports outside macro arguments, then pass their values"
    )]
    MacroExecutionBoundary,
    /// A ref uses positioning without a known anchor attachment.
    #[error("ref `{name}` uses position without an anchor projection")]
    MissingAnchor {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
    },
    /// A ref uses a container query without a known container attachment.
    #[error("ref `{name}` uses a container query without a container projection")]
    MissingContainer {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
    },
    /// A selector has known ref attachments, but none is an ancestor in its JSX tree.
    #[error("ref `{name}` selector has no known ancestor ref in this JSX tree")]
    MissingRefAncestor {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
    },
    /// A bounded ref's boundary has no known JSX attachment.
    #[error("ref `{name}` is bounded by `{boundary}`, but no element attaches that boundary")]
    UnattachedBoundary {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
        /// Authored name of the required boundary ref.
        boundary: String,
    },
    /// A bounded ref is attached outside its boundary's ancestry.
    #[error("the boundary `{boundary}` must be attached to an ancestor of ref `{name}`")]
    MissingBoundaryAncestor {
        /// Authored name of the required boundary ref.
        boundary: String,
        /// Authored or inferred ref binding name used for reporting.
        name: String,
    },
    /// A bounded selector targets an element outside its boundary's ancestry.
    #[error(
        "a selector using bounded ref `{name}` must target an element within boundary `{boundary}`"
    )]
    SelectorOutsideBoundary {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
        /// Authored name of the required boundary ref.
        boundary: String,
    },
    /// A Tailwind relationship utility violates the active typed-relationship policy.
    #[error(
        "Tailwind relationship utility `{utility}` is forbidden; use style.ref(), optionally bounded by another ref"
    )]
    PreferTypedRelationships {
        /// Utility text as authored, including variants.
        utility: String,
    },
    /// A style rule would apply to the document instead of to the styled element.
    #[error(
        "`{selector}` does not depend on the styled element; declare document-wide CSS in a global stylesheet"
    )]
    DocumentScopedSelector {
        /// Resolved selector that references neither the element nor a declaration identity.
        selector: String,
    },
    /// A style call marks participants with a transition class but names none.
    #[error(
        "a view transition class without a name does not participate; add .for(key) or declare view-transition-name"
    )]
    MissingViewTransitionName,
    /// A style call supplies more than one view-transition-name for one element.
    #[error(
        "{count} view-transition-name sources in one style() call; only one element value applies, and a runtime name wins"
    )]
    DuplicateViewTransitionName {
        /// How many sources the call supplied.
        count: usize,
    },
    /// An element's view-transition classes come from more than one style call.
    #[error(
        "view-transition-class is one property: this call's list replaces the one its composed base declares, so declare the transitions in one call"
    )]
    SplitViewTransitionClasses,
    /// Two composition bases declare one declaration slot and neither precedes the other.
    #[error(
        "two composition bases declare `{properties}`; neither base precedes the other, so which declaration wins is undefined"
    )]
    AmbiguousBaseOrder {
        /// The competing declarations, as `property` or `property and property` when one
        /// side is a shorthand of the other.
        properties: String,
    },
    /// A base owned by another module declares a property this call also declares.
    #[error("the base in `{module}` overlaps these declarations: `{properties}`")]
    ImportedBaseInterference {
        /// Host identity of the module owning the base.
        module: ModuleId,
        /// The competing declarations, as `property` or `property and property`.
        properties: String,
    },
    /// A host generator failed or returned a body its write positions cannot hold.
    #[error("identifier generator for {kind} was rejected: {detail}")]
    RejectedIdentifier {
        /// Identifier family the generator was asked for.
        kind: String,
        /// Generator failure message, or the body that failed validation.
        detail: String,
    },
    /// A utility literal is retained as an external class because resolution failed.
    #[error("unknown or unavailable utility `{utility}`")]
    UnresolvedUtility {
        /// Utility text as authored, including variants.
        utility: String,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
    /// An editor candidate cannot be resolved against the pinned design system.
    #[error("unresolved candidate: {candidate}")]
    UnresolvedCandidate {
        /// Candidate text supplied to the query.
        candidate: String,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
    /// A candidate preview cannot be split into complete generated CSS blocks.
    #[error("invalid generated CSS block at byte {offset}")]
    InvalidCandidateCss {
        /// Zero-based UTF-8 byte offset within the offending generated CSS contribution.
        offset: usize,
        /// Whether the candidate syntax is valid; determines the lint code.
        classification: UtilityFailure,
    },
}

impl LintKind {
    /// Returns the stable reporting code; lint codes also name suppression rules.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MacroArgumentMutation => "macro-argument-mutation",
            Self::MacroExecutionBoundary => "macro-execution-boundary",
            Self::MissingAnchor { .. } => "require-anchor-provider",
            Self::MissingContainer { .. } => "require-container-provider",
            Self::MissingRefAncestor { .. } => "valid-ref-ancestry",
            Self::UnattachedBoundary { .. } => "require-boundary-provider",
            Self::MissingBoundaryAncestor { .. } => "valid-boundary-ancestry",
            Self::SelectorOutsideBoundary { .. } => "bounded-selector-in-boundary",
            Self::PreferTypedRelationships { .. } => "prefer-typed-relationships",
            Self::DocumentScopedSelector { .. } => "require-element-scope",
            Self::MissingViewTransitionName => "require-view-transition-name",
            Self::DuplicateViewTransitionName { .. } => "single-view-transition-name",
            Self::SplitViewTransitionClasses => "single-view-transition-class",
            Self::AmbiguousBaseOrder { .. } => "ambiguous-base-order",
            Self::ImportedBaseInterference { .. } => "imported-base-interference",
            Self::RejectedIdentifier { .. } => "rejected-identifier",
            Self::UnresolvedUtility { classification, .. }
            | Self::UnresolvedCandidate { classification, .. }
            | Self::InvalidCandidateCss { classification, .. } => classification.code(),
        }
    }
}

/// One suppressible finding with its source highlights and supplementary context.
///
/// [`Self::new`] attaches one primary location. Secondary labels and notes can be added
/// without changing the cause. Suppression uses only the first label, or source offset
/// zero when no labels remain; it does not inspect notes or secondary highlights.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("{kind}")]
pub struct LintFinding {
    /// Typed cause; owns the message template and reporting code.
    #[source]
    pub kind: LintKind,
    /// Mandatory primary location and shared diagnostic details.
    pub info: Box<DiagnosticInfo>,
}

impl LintFinding {
    /// Attaches one primary source span to a cause, with no secondary labels or notes.
    pub fn new(kind: LintKind, span: Span) -> Self {
        Self {
            kind,
            info: Box::new(DiagnosticInfo::new(span)),
        }
    }

    /// Returns the cause's stable reporting code.
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl std::ops::Deref for LintFinding {
    type Target = DiagnosticInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl std::ops::DerefMut for LintFinding {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

/// Syntax classification of an unresolved utility or candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum UtilityFailure {
    /// Candidate syntax is malformed (`invalid-utility`).
    Invalid,
    /// Syntax is valid, but no available utility resolves it (`unknown-utility`).
    Unknown,
}

impl UtilityFailure {
    /// Returns the stable lint rule name for this classification.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "invalid-utility",
            Self::Unknown => "unknown-utility",
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::SourceError;

    use super::*;
    use crate::{CompileErrorKind, SourceFailureContext};

    /// One message template can carry several codes, including codes shared with fatal
    /// errors for the same source-coordinate cause.
    #[test]
    fn one_message_template_can_report_multiple_codes() {
        let findings = [UtilityFailure::Invalid, UtilityFailure::Unknown].map(|classification| {
            LintKind::UnresolvedUtility {
                utility: "hover:[".into(),
                classification,
            }
        });
        assert_eq!(findings[0].code(), "invalid-utility");
        assert_eq!(findings[1].code(), "unknown-utility");
        assert_eq!(findings[0].to_string(), findings[1].to_string());
        assert_eq!(
            findings[0].to_string(),
            "unknown or unavailable utility `hover:[`"
        );

        for (context, code) in [
            (SourceFailureContext::Utility, "SHSPAN001"),
            (SourceFailureContext::Projection, "SHPRJ001"),
            (SourceFailureContext::Stylesheet, "SHCSS001"),
        ] {
            let error = CompileErrorKind::source_failure(
                &SourceError::InvalidRange { start: 9, end: 2 },
                context,
            );
            assert_eq!(error.code(), code);
            assert_eq!(error.to_string(), "invalid source byte range 9..2");
        }
    }
}
