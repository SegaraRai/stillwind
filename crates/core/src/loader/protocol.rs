use super::{Arc, CompilerOptions, DesignSystemSnapshot, FileId, SourceFile};
pub type DesignSystemOptions = CompilerOptions;

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
}

#[derive(Clone, Debug)]
pub struct CssSourceMapping {
    pub generated: std::ops::Range<usize>,
    pub file: FileId,
    pub source_start: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesignSystemLoadError {
    RequestIdExhausted,
    InvalidCss {
        source: CssSourceName,
        message: String,
    },
    Protocol {
        message: String,
    },
    Cycle {
        chain: Vec<CssFileRequest>,
    },
}

impl std::fmt::Display for DesignSystemLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestIdExhausted => f.write_str("CSS request IDs exhausted"),
            Self::InvalidCss { source, message } => write!(f, "{}: {message}", source.0),
            Self::Protocol { message } => write!(f, "CSS loader protocol error: {message}"),
            Self::Cycle { chain } => write!(f, "CSS import cycle: {chain:?}"),
        }
    }
}
impl std::error::Error for DesignSystemLoadError {}

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
