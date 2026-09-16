//! Advisory causes, suppression rule names, and source-located findings.
//!
//! Construct [`LintFinding`] from a [`LintKind`] and the reported source span.
//! [`UtilityFailure`] distinguishes malformed candidate syntax from unresolved names;
//! detection and host severity policy remain outside this module.

use serde::{Deserialize, Serialize};

use stillwind_source::Span;

use crate::Label;

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
    /// A scoped ref's provider has no known JSX attachment.
    #[error(
        "ref `{name}` is bound to subtree `{subtree}`, but that subtree is not attached to a JSX element"
    )]
    UnattachedSubtree {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
        /// Authored name of the required subtree.
        subtree: String,
    },
    /// A scoped ref is attached outside its provider's ancestry.
    #[error("subtree `{subtree}` must be attached to an ancestor of ref `{name}`")]
    MissingSubtreeAncestor {
        /// Authored name of the required subtree.
        subtree: String,
        /// Authored or inferred ref binding name used for reporting.
        name: String,
    },
    /// A subtree-bound selector targets an element outside its provider's ancestry.
    #[error(
        "a selector using a subtree-bound ref `{name}` must target an element inside subtree `{subtree}`"
    )]
    SelectorOutsideSubtree {
        /// Authored or inferred ref binding name used for reporting.
        name: String,
        /// Authored name of the required subtree.
        subtree: String,
    },
    /// A Tailwind relationship utility violates the active typed-relationship policy.
    #[error(
        "Tailwind relationship utility `{utility}` is forbidden; use style.ref() or style.subtree()"
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
            Self::UnattachedSubtree { .. } => "require-scope-provider",
            Self::MissingSubtreeAncestor { .. } => "valid-scope-ancestry",
            Self::SelectorOutsideSubtree { .. } => "scoped-selector-in-scope",
            Self::PreferTypedRelationships { .. } => "prefer-typed-relationships",
            Self::DocumentScopedSelector { .. } => "require-element-scope",
            Self::MissingViewTransitionName => "require-view-transition-name",
            Self::DuplicateViewTransitionName { .. } => "single-view-transition-name",
            Self::SplitViewTransitionClasses => "single-view-transition-class",
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind}")]
pub struct LintFinding {
    /// Typed cause; owns the message template and reporting code.
    #[source]
    pub kind: LintKind,
    /// Ordered source highlights. An empty list uses the whole source for suppression.
    pub labels: Vec<Label>,
    /// Ordered supplementary explanations, formatted separately from the primary message.
    pub notes: Vec<String>,
}

impl LintFinding {
    /// Attaches one primary source span to a cause, with no secondary labels or notes.
    pub fn new(kind: LintKind, span: Span) -> Self {
        Self {
            kind,
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
        }
    }

    /// Returns the cause's stable reporting code.
    pub const fn code(&self) -> &'static str {
        self.kind.code()
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
