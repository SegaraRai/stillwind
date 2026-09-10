use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_css::{ThemeError, parse_theme};
use stillwind_source::{FileId, SourceError, SourceFile};

use crate::{CompilerOptions, CompilerOptionsError, DesignSystemSnapshot};

use super::CssImportError;
use super::expansion::ExpandedSource;
use super::imports::{Import, imports};
use super::protocol::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssSource, CssSourceEdge, CssSourceGraph,
    CssSourceName, DesignSystemLoadStep,
};

#[derive(Clone, Debug, thiserror::Error)]
pub enum DesignSystemLoadError {
    #[error("inconsistent CSS source graph")]
    InvalidGraph,
    #[error("{0}")]
    Source(#[from] SourceError),
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
        cause: ThemeError,
    },
    #[error("{}: {cause}", .name.0)]
    InvalidImport {
        name: CssSourceName,
        #[source]
        cause: CssImportError,
    },
    #[error("{0}")]
    InvalidOptions(#[from] CompilerOptionsError),
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
    options: CompilerOptions,
    graph: CssSourceGraph,
    known: BTreeMap<CssSourceName, usize>,
    nodes: Vec<Node>,
    pending: BTreeMap<u32, (usize, usize)>,
    next_id: u32,
    terminal: bool,
}

impl DesignSystemLoader {
    pub fn new(options: CompilerOptions) -> Self {
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
        let mut ids = BTreeSet::new();
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
                *self
                    .nodes
                    .get_mut(source)
                    .and_then(|node| node.targets.get_mut(import))
                    .ok_or(DesignSystemLoadError::InvalidGraph)? = Some(index);
                self.graph.edges.push(CssSourceEdge {
                    request: self
                        .nodes
                        .get(source)
                        .and_then(|node| node.requests.get(import))
                        .ok_or(DesignSystemLoadError::InvalidGraph)?
                        .clone(),
                    target: file.name,
                });
            }
        }
        // Also detect cycles in resolved subgraphs while unrelated I/O is outstanding.
        let order = self.source_order()?;
        if !self.pending.is_empty() {
            return Ok(DesignSystemLoadStep::NeedFile(
                self.pending
                    .values()
                    .map(|&(source, import)| {
                        self.nodes
                            .get(source)
                            .and_then(|node| node.requests.get(import))
                            .cloned()
                            .ok_or(DesignSystemLoadError::InvalidGraph)
                    })
                    .collect::<Result<_, _>>()?,
            ));
        }
        let output = self.expand(&order)?;
        let name = self
            .graph
            .sources
            .first()
            .ok_or(DesignSystemLoadError::InvalidGraph)?
            .name
            .clone();
        let source = SourceFile::new(FileId(u32::MAX), name.0.clone(), output.text.as_str())?;
        let mut theme =
            parse_theme(&source).map_err(|error| DesignSystemLoadError::InvalidCss {
                name: name.clone(),
                cause: error,
            })?;
        theme
            .output_template
            .map_origins(|span| output.original_span(span));
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
        self.graph.resources = output.resources;
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
            if self
                .graph
                .sources
                .get(index)
                .ok_or(DesignSystemLoadError::InvalidGraph)?
                .source
                .text()
                != content.as_ref()
            {
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
        let source = Arc::new(SourceFile::new(FileId(id), name.0.clone(), content)?);
        parse_theme(&source).map_err(|error| DesignSystemLoadError::InvalidCss {
            name: name.clone(),
            cause: error,
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

    /// Postorder traversal using heap frames, including partially resolved graphs.
    fn source_order(&self) -> Result<Vec<usize>, DesignSystemLoadError> {
        let mut state = vec![0; self.nodes.len()];
        *state
            .first_mut()
            .ok_or(DesignSystemLoadError::InvalidGraph)? = 1;
        let mut frames = vec![(0, 0)];
        let mut chain = Vec::new();
        let mut order = Vec::new();
        while let Some((index, next)) = frames.last_mut() {
            let node = self
                .nodes
                .get(*index)
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            if node.requests.len() != node.targets.len() {
                return Err(DesignSystemLoadError::InvalidGraph);
            }
            let Some(target) = node.targets.get(*next) else {
                *state
                    .get_mut(*index)
                    .ok_or(DesignSystemLoadError::InvalidGraph)? = 2;
                order.push(*index);
                frames.pop();
                chain.pop();
                continue;
            };
            let request = node
                .requests
                .get(*next)
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            *next += 1;
            let Some(target) = target else { continue };
            let mark = state
                .get_mut(*target)
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            if *mark == 1 {
                chain.push(request.clone());
                return Err(DesignSystemLoadError::Cycle { chain });
            }
            if *mark == 0 {
                *mark = 1;
                chain.push(request.clone());
                frames.push((*target, 0));
            }
        }
        Ok(order)
    }

    fn expand(&self, order: &[usize]) -> Result<ExpandedSource, DesignSystemLoadError> {
        let mut cache: BTreeMap<usize, ExpandedSource> = BTreeMap::new();
        for &index in order {
            let source = &self
                .graph
                .sources
                .get(index)
                .ok_or(DesignSystemLoadError::InvalidGraph)?
                .source;
            let node = self
                .nodes
                .get(index)
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            let mut output = ExpandedSource::default();
            let mut cursor = 0;
            for (import, target) in node.imports.iter().zip(&node.targets) {
                output.append_source(source, cursor..import.start)?;
                let target = target.ok_or(DesignSystemLoadError::InvalidGraph)?;
                let imported = cache
                    .get(&target)
                    .ok_or(DesignSystemLoadError::InvalidGraph)?;
                output.append_import(imported, &import.wrappers)?;
                cursor = import.end;
            }
            output.append_source(source, cursor..source.text().len())?;
            cache.insert(index, output);
        }
        cache.remove(&0).ok_or(DesignSystemLoadError::InvalidGraph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain_loader(depth: usize) -> DesignSystemLoader {
        let mut loader = DesignSystemLoader::new(CompilerOptions::default());
        for index in 0..depth {
            let text = if index + 1 < depth {
                "@import 'next';"
            } else {
                ".x{color:red;}"
            };
            let source = Arc::new(
                SourceFile::new(FileId(u32::try_from(index + 1).unwrap()), "node.css", text)
                    .unwrap(),
            );
            let imports = imports(text).unwrap();
            let requests = imports
                .iter()
                .map(|_| CssFileRequest {
                    id: u32::try_from(index + 1).unwrap(),
                    importer: "node.css".into(),
                    specifier: "next".into(),
                    kind: CssDependencyKind::Import,
                })
                .collect();
            let targets = imports.iter().map(|_| Some(index + 1)).collect();
            loader.nodes.push(Node {
                imports,
                requests,
                targets,
            });
            loader.graph.sources.push(CssSource {
                name: "node.css".into(),
                source,
            });
        }
        loader
    }

    #[test]
    fn deep_import_graphs_use_heap_frames() {
        let loader = chain_loader(20_000);
        let order = loader.source_order().unwrap();
        assert_eq!(order, (0..20_000).rev().collect::<Vec<_>>());
        let loader = chain_loader(512);
        let output = loader.expand(&loader.source_order().unwrap()).unwrap();
        assert_eq!(output.text, ".x{color:red;}");
    }

    #[test]
    fn incomplete_expansion_returns_a_graph_error() {
        let mut loader = chain_loader(2);
        loader.nodes[0].targets[0] = None;
        assert!(matches!(
            loader.expand(&loader.source_order().unwrap()),
            Err(DesignSystemLoadError::InvalidGraph)
        ));
    }

    #[test]
    fn request_id_exhaustion_is_terminal_and_never_emits_max_or_wraps() {
        let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
        let mut loader = DesignSystemLoader::new(CompilerOptions::default());
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
