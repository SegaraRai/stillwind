use super::{
    Arc, BTreeMap, CssDependencyKind, CssFileInput, CssFileRequest, CssSource, CssSourceEdge,
    CssSourceGraph, CssSourceName, DesignSystemLoadStep, DesignSystemOptions, DesignSystemSnapshot,
    ExpandedSource, FileId, Import, SourceFile, imports,
};

#[derive(Clone, Debug, thiserror::Error)]
pub enum DesignSystemLoadError {
    #[error("CSS request IDs exhausted")]
    RequestIdExhausted,
    #[error("too many CSS sources")]
    SourceIdExhausted,
    #[error("provide called after completion")]
    AlreadyComplete,
    #[error("initial provide must contain exactly one file (conventionally ID 0), got {count}")]
    InvalidInitialBatch { count: usize },
    #[error("unknown or duplicate request ID {id}")]
    UnknownOrDuplicateRequest { id: u32 },
    #[error("conflicting content for {}", .name.0)]
    ConflictingContent { name: CssSourceName },
    #[error("{}: {cause}", .name.0)]
    InvalidCss {
        name: CssSourceName,
        #[source]
        cause: stillwind_css::ThemeError,
    },
    #[error("{}: {cause}", .name.0)]
    InvalidImport {
        name: CssSourceName,
        #[source]
        cause: super::CssImportError,
    },
    #[error("{0}")]
    InvalidOptions(#[from] crate::CompilerOptionsError),
    #[error("CSS import cycle: {chain:?}")]
    Cycle { chain: Vec<CssFileRequest> },
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

impl DesignSystemLoader {
    pub fn new(options: DesignSystemOptions) -> Self {
        Self {
            options,
            graph: CssSourceGraph::default(),
            known: BTreeMap::new(),
            nodes: Vec::new(),
            pending: BTreeMap::new(),
            next_id: 1,
            terminal: false,
        }
    }

    pub fn provide(
        &mut self,
        id: u32,
        name: CssSourceName,
        content: Arc<str>,
    ) -> DesignSystemLoadStep {
        self.provide_many([CssFileInput { id, name, content }])
    }

    pub fn provide_many(
        &mut self,
        files: impl IntoIterator<Item = CssFileInput>,
    ) -> DesignSystemLoadStep {
        if self.terminal {
            return DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::AlreadyComplete));
        }
        match self.accept_many(files.into_iter().collect()) {
            Ok(step) => step,
            Err(error) => {
                self.terminal = true;
                DesignSystemLoadStep::Complete(Err(error))
            }
        }
    }

    fn accept_many(
        &mut self,
        files: Vec<CssFileInput>,
    ) -> Result<DesignSystemLoadStep, DesignSystemLoadError> {
        let initial = self.nodes.is_empty();
        if initial && files.len() != 1 {
            return Err(DesignSystemLoadError::InvalidInitialBatch { count: files.len() });
        }
        // Validate against requests issued before this batch, including duplicates.
        let mut ids = std::collections::BTreeSet::new();
        for file in &files {
            if !ids.insert(file.id) || (!initial && !self.pending.contains_key(&file.id)) {
                return Err(DesignSystemLoadError::UnknownOrDuplicateRequest { id: file.id });
            }
        }
        for file in files {
            let parent = if initial {
                None
            } else {
                self.pending.remove(&file.id)
            };
            let index = self.accept_source(file.name.clone(), file.content)?;
            if let Some((source, import)) = parent {
                self.nodes[source].targets[import] = Some(index);
                self.graph.edges.push(CssSourceEdge {
                    request: self.nodes[source].requests[import].clone(),
                    target: file.name,
                });
            }
        }
        // Also detect cycles in resolved subgraphs while unrelated I/O is outstanding.
        let mut state = vec![0; self.nodes.len()];
        self.check_cycles(0, &mut state, &mut Vec::new())?;
        if !self.pending.is_empty() {
            return Ok(DesignSystemLoadStep::NeedFile(
                self.pending
                    .values()
                    .map(|&(source, import)| self.nodes[source].requests[import].clone())
                    .collect(),
            ));
        }
        let output = self.expand(0, &mut BTreeMap::new());
        let name = self.graph.sources[0].name.clone();
        let source = SourceFile::new(FileId(u32::MAX), name.0.clone(), output.text.as_str());
        let mut theme = stillwind_css::parse_tailwind_theme(&source).map_err(|error| {
            DesignSystemLoadError::InvalidCss {
                name: name.clone(),
                cause: error,
            }
        })?;
        for token in &mut theme.tokens {
            if token.name.span.file == source.id() {
                if let Some(span) = output.original_span(token.name.span) {
                    token.name.span = span;
                }
                if let Some(span) = output.original_span(token.value.span) {
                    token.value.span = span;
                }
            }
        }
        self.graph.mappings = output.mappings;
        let snapshot = DesignSystemSnapshot::from_loaded_theme(
            theme,
            self.options.clone(),
            std::mem::take(&mut self.graph),
        )
        .map_err(DesignSystemLoadError::InvalidOptions)?;
        self.terminal = true;
        Ok(DesignSystemLoadStep::Complete(Ok(snapshot)))
    }

    fn accept_source(
        &mut self,
        name: CssSourceName,
        content: Arc<str>,
    ) -> Result<usize, DesignSystemLoadError> {
        if let Some(&index) = self.known.get(&name) {
            if self.graph.sources[index].source.text() != content.as_ref() {
                return Err(DesignSystemLoadError::ConflictingContent { name: name.clone() });
            }
            return Ok(index);
        }
        let imports = imports(&content).map_err(|cause| DesignSystemLoadError::InvalidImport {
            name: name.clone(),
            cause,
        })?;
        let index = self.nodes.len();
        let id = u32::try_from(index + 1).map_err(|_| DesignSystemLoadError::SourceIdExhausted)?;
        let source = Arc::new(SourceFile::new(FileId(id), name.0.clone(), content));
        stillwind_css::parse_tailwind_theme(&source).map_err(|error| {
            DesignSystemLoadError::InvalidCss {
                name: name.clone(),
                cause: error,
            }
        })?;
        let mut requests = Vec::new();
        for (import_index, import) in imports.iter().enumerate() {
            let id = self.next_id;
            self.next_id = id
                .checked_add(1)
                .ok_or(DesignSystemLoadError::RequestIdExhausted)?;
            requests.push(CssFileRequest {
                id,
                importer: name.clone(),
                specifier: import.specifier.clone(),
                kind: CssDependencyKind::Import,
            });
            self.pending.insert(id, (index, import_index));
        }
        self.nodes.push(Node {
            targets: vec![None; imports.len()],
            imports,
            requests,
        });
        self.known.insert(name.clone(), index);
        self.graph.sources.push(CssSource { name, source });
        Ok(index)
    }

    fn check_cycles(
        &self,
        index: usize,
        state: &mut [u8],
        chain: &mut Vec<CssFileRequest>,
    ) -> Result<(), DesignSystemLoadError> {
        if state[index] == 1 {
            return Err(DesignSystemLoadError::Cycle {
                chain: chain.clone(),
            });
        }
        if state[index] == 2 {
            return Ok(());
        }
        state[index] = 1;
        for (import, target) in self.nodes[index].targets.iter().enumerate() {
            if let Some(target) = target {
                chain.push(self.nodes[index].requests[import].clone());
                self.check_cycles(*target, state, chain)?;
                chain.pop();
            }
        }
        state[index] = 2;
        Ok(())
    }

    fn expand(&self, index: usize, cache: &mut BTreeMap<usize, ExpandedSource>) -> ExpandedSource {
        if let Some(output) = cache.get(&index) {
            return output.clone();
        }
        let source = &self.graph.sources[index].source;
        let node = &self.nodes[index];
        let mut output = ExpandedSource::default();
        let mut cursor = 0;
        for (import, target) in node.imports.iter().zip(&node.targets) {
            output.append_source(source, cursor..import.start);
            output.append_import(
                &self.expand(target.expect("all requests resolved"), cache),
                &import.wrappers,
            );
            cursor = import.end;
        }
        output.append_source(source, cursor..source.text().len());
        cache.insert(index, output.clone());
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_id_exhaustion_is_terminal_and_never_emits_max_or_wraps() {
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
        loader.next_id = u32::MAX - 1;
        let DesignSystemLoadStep::NeedFile(requests) =
            loader.provide(0, "root".into(), "@import 'last';".into())
        else {
            panic!("expected last usable request ID");
        };
        assert_eq!(requests[0].id, u32::MAX - 1);
        assert!(matches!(
            loader.provide(u32::MAX - 1, "last".into(), "@import 'overflow';".into()),
            DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
        ));
        assert_eq!(loader.next_id, u32::MAX);
        assert!(matches!(
            loader.provide_many([]),
            DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::AlreadyComplete))
        ));
    }

    #[test]
    fn exhausted_batch_finishes_with_error_instead_of_returning_partial_requests() {
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
        loader.next_id = u32::MAX - 1;
        assert!(matches!(
            loader.provide(
                0,
                "root".into(),
                "@import 'last'; @import 'overflow';".into()
            ),
            DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
        ));
        assert_eq!(loader.next_id, u32::MAX);
    }
}
