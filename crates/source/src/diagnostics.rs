use serde::{Deserialize, Serialize};

use super::text::Span;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileError {
    pub code: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl CompileError {
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LintFinding {
    pub rule: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl LintFinding {
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
