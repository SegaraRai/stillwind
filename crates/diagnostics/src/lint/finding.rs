//! Source labels and supplementary details attached to one detected lint.
//!
//! [`LintFinding::new`] attaches a primary span to a [`LintDiagnostic`]. Suppression
//! uses the primary label; reporters resolve labels and apply reporting settings.

use serde::Serialize;

use stillwind_source::Span;

use super::{LintDiagnostic, LintKind};
use crate::DiagnosticInfo;

/// One suppressible finding with its source highlights and supplementary context.
///
/// [`Self::new`] attaches one primary location. Secondary labels and notes can be added
/// without changing the cause. Suppression uses only the first label, or source offset
/// zero when no labels remain; it does not inspect notes or secondary highlights.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("{diagnostic}")]
pub struct LintFinding {
    /// Typed cause; owns the message template and reporting code.
    #[source]
    pub diagnostic: LintDiagnostic,
    /// Mandatory primary location and shared diagnostic details.
    pub info: Box<DiagnosticInfo>,
}

impl LintFinding {
    /// Returns a finding containing `diagnostic` and one primary label at `span`.
    /// The caller must supply a span in the source used to report or suppress the finding;
    /// this function does not validate its file identity or bounds. Notes and actions start empty.
    pub fn new(diagnostic: LintDiagnostic, span: Span) -> Self {
        Self {
            diagnostic,
            info: Box::new(DiagnosticInfo::new(span)),
        }
    }

    /// Returns the rule identified by this diagnostic.
    pub const fn kind(&self) -> LintKind {
        self.diagnostic.kind()
    }

    /// Returns the cause's stable reporting code.
    pub const fn code(&self) -> &'static str {
        self.kind().name()
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
