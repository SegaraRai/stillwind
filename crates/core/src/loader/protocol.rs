use std::sync::Arc;

use stillwind_source::{FileId, SourceFile};

use crate::DesignSystemSnapshot;

use super::state::DesignSystemLoadError;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CssSourceName(pub Arc<str>);

impl From<&str> for CssSourceName {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssDependencyKind {
    Import,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssFileRequest {
    pub id: u32,
    pub importer: CssSourceName,
    pub specifier: String,
    pub kind: CssDependencyKind,
}

#[derive(Clone, Debug)]
pub struct CssSource {
    pub name: CssSourceName,
    pub source: Arc<SourceFile>,
}

#[derive(Clone, Debug)]
pub struct CssSourceEdge {
    pub request: CssFileRequest,
    pub target: CssSourceName,
}

#[derive(Clone, Debug, Default)]
pub struct CssSourceGraph {
    pub sources: Vec<CssSource>,
    pub edges: Vec<CssSourceEdge>,
    pub mappings: Vec<CssSourceMapping>,
    pub resources: Vec<CssResourceReference>,
}

/// A resource placeholder in generated CSS. Hosts resolve specifiers relative
/// to `owner`, then replace the placeholder before their CSS asset pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssResourceReference {
    pub placeholder: String,
    pub owner: String,
    pub specifier: String,
    pub span: stillwind_source::Span,
}

#[derive(Clone, Debug)]
pub struct CssSourceMapping {
    pub generated: std::ops::Range<usize>,
    pub file: FileId,
    pub source_start: usize,
    /// Explicit original end for a length-changing resource substitution.
    pub source_end: Option<usize>,
}

#[derive(Debug)]
pub enum DesignSystemLoadStep {
    NeedFile(Vec<CssFileRequest>),
    Complete(Result<DesignSystemSnapshot, DesignSystemLoadError>),
}

/// A response to a request ID. The initial batch contains exactly one root source.
#[derive(Clone, Debug)]
pub struct CssFileInput {
    pub id: u32,
    pub name: CssSourceName,
    pub content: Arc<str>,
}
