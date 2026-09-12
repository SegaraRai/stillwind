//! Errors and lint findings shared by every compiler pass.
//!
//! Both types carry a stable machine-readable name (`code` for errors, `rule` for
//! findings) as `&'static str`, so hosts can inspect codes instead of parsing messages.
//! These strings are not a closed enum; callers can construct additional codes. Each
//! diagnostic additionally carries ordered `labels` that point into the `SourceFile`
//! which produced it, plus free-form `notes`.
//!
//! The label order is significant: `filter_lint_findings` uses the first label to
//! decide which line a suppression comment applies to, and host renderers use it as the
//! primary location. Span arithmetic is never done here; spans are produced and
//! validated by the pass that raises the diagnostic.

use serde::{Deserialize, Serialize};

use crate::text::Span;

/// One highlighted source range with an optional local explanation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    /// Range to highlight. It belongs to the file that raised the diagnostic.
    pub span: Span,
    /// Text shown next to the range; `None` lets the diagnostic message stand for the
    /// label itself.
    pub message: Option<String>,
}

/// A compilation error: something the compiler refuses to compile.
///
/// Errors are values rather than panics so hosts can render them, and so one pass can
/// collect several before failing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileError {
    /// Stable code such as `SHJS001`; hosts match on it, never on `message`.
    pub code: &'static str,
    /// Human-readable summary, already formatted for display.
    pub message: String,
    /// Ordered highlight ranges; empty only if a caller bypassed `new`.
    pub labels: Vec<Label>,
    /// Extra context printed after the labels.
    pub notes: Vec<String>,
}

impl CompileError {
    /// Creates an error with one unlabeled span and no notes.
    pub fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            message: message.into(),
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
        }
    }
}

/// A lint finding: the code compiles, but a rule reports it.
///
/// Findings are advisory and may be removed by `swlint-disable` comments; errors never
/// are.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LintFinding {
    /// Stable rule name as used by `swlint-*` directives.
    pub rule: &'static str,
    /// Human-readable summary, already formatted for display.
    pub message: String,
    /// Ordered highlight ranges; empty only if a caller bypassed `new`.
    pub labels: Vec<Label>,
    /// Extra context printed after the labels.
    pub notes: Vec<String>,
}

impl LintFinding {
    /// Creates a finding with one unlabeled span and no notes.
    pub fn new(rule: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            rule,
            message: message.into(),
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
        }
    }
}
