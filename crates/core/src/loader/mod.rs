//! Caller-driven CSS loading. Source identities are deliberately opaque.
use std::{collections::BTreeMap, sync::Arc};

use cssparser::{Parser, Token};
use stillwind_source::{FileId, SourceFile, Span};

use crate::{CompilerOptions, DesignSystemSnapshot};

mod protocol;
pub use protocol::*;
mod state;
pub use state::DesignSystemLoadError;
#[derive(Clone, Debug)]
struct Import {
    start: usize,
    end: usize,
    specifier: String,
    wrappers: Vec<String>,
}

struct Node {
    imports: Vec<Import>,
    requests: Vec<CssFileRequest>,
    targets: Vec<Option<usize>>,
}

/// Caller-driven loading with independent, monotonically increasing request IDs.
/// `NeedFile` lists all outstanding requests; hosts may respond in any order.
pub struct DesignSystemLoader {
    options: DesignSystemOptions,
    graph: CssSourceGraph,
    known: BTreeMap<CssSourceName, usize>,
    nodes: Vec<Node>,
    pending: BTreeMap<u32, (usize, usize)>,
    next_id: u32,
    terminal: bool,
}

mod expansion;
use expansion::ExpandedSource;
mod imports;
pub use imports::CssImportError;
use imports::imports;
#[cfg(test)]
mod tests;
