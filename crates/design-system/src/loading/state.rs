//! The loader state machine and its failures.
//!
//! [`DesignSystemLoader`] accepts responses, parses each supplied buffer immediately,
//! and reports either the outstanding requests or one final snapshot. Expansion happens
//! only when nothing is pending: sources are concatenated in postorder so an imported
//! file's text appears where its authored `@import` was, then the whole result is
//! parsed once, so theme tokens see the imported definitions and authored order is
//! preserved regardless of the order in which reads finished.
//!
//! A source name is the loader's unit of identity and cycle detection. Supplying a
//! known name again is allowed only with byte-identical content. Failures the loader
//! can decide from responses (unknown IDs, invalid CSS or imports, cycles, conflicting
//! content, exhausted IDs) become
//! [`Complete(Err(..))`](DesignSystemLoadStep::Complete); a host that cannot read a
//! requested source drops the loader instead, so no host failure is ever reported as a
//! loader error.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_source::{FileId, SourceError, SourceFile};

use super::CssImportError;
use super::expansion::ExpandedSource;
use super::imports::{Import, imports};
use super::protocol::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssSource, CssSourceEdge, CssSourceGraph,
    CssSourceName, DesignSystemLoadStep,
};
use crate::{
    DesignSystemOptions, DesignSystemOptionsError, DesignSystemSnapshot, ThemeError, parse_theme,
};

/// Why a load could not produce a snapshot.
///
/// Every variant is decidable from the buffers the host supplied, except
/// [`Self::Source`], which reports a buffer that exceeded the source size
/// limit. A host resolution or read failure is not a loader error: the host
/// reports it with the request's importer and specifier and drops the loader.
#[derive(Clone, Debug, thiserror::Error)]
pub enum DesignSystemLoadError {
    /// The retained graph contradicts the retained nodes.
    #[error("inconsistent CSS source graph")]
    InvalidGraph,
    /// A supplied buffer could not be stored as a source file.
    #[error("{0}")]
    Source(#[from] SourceError),
    /// The monotonic request counter reached `u32::MAX`.
    #[error("CSS request IDs exhausted")]
    RequestIdExhausted,
    /// The graph holds more sources than a 32-bit file ID can address.
    #[error("too many CSS sources")]
    SourceIdExhausted,
    /// A response arrived after the load had already completed.
    #[error("provide called after completion")]
    AlreadyComplete,
    /// The first batch did not contain exactly one root source.
    #[error("initial provide must contain exactly one file (conventionally ID 0), got {count}")]
    InvalidInitialBatch {
        /// Number of files the first batch contained.
        count: usize,
    },
    /// A response ID was never issued, or was answered twice.
    #[error("unknown or duplicate request ID {id}")]
    UnknownOrDuplicateRequest {
        /// The offending response ID.
        id: u32,
    },
    /// A known name was supplied again with different content.
    #[error("conflicting content for {}", .name.0)]
    ConflictingContent {
        /// Name that was supplied twice.
        name: CssSourceName,
    },
    /// A supplied buffer is not valid Stillwind CSS.
    #[error("{}: {cause}", .name.0)]
    InvalidCss {
        /// Name the buffer was supplied under.
        name: CssSourceName,
        #[source]
        /// Parser failure for that buffer.
        cause: ThemeError,
    },
    /// A supplied `@import` could not be parsed.
    #[error("{}: {cause}", .name.0)]
    InvalidImport {
        /// Name the buffer was supplied under.
        name: CssSourceName,
        #[source]
        /// Import parser failure for that buffer.
        cause: CssImportError,
    },
    /// The resolved prefix configuration is invalid.
    #[error("{0}")]
    InvalidOptions(#[from] DesignSystemOptionsError),
    /// The import graph contains a cycle; `chain` lists the requests that
    /// close it, in traversal order.
    #[error("CSS import cycle: {chain:?}")]
    Cycle {
        /// Requests that close the cycle, in traversal order.
        chain: Vec<CssFileRequest>,
    },
}

/// One accepted source: its imports, the requests issued for them, and the
/// resolved target index per import slot.
struct Node {
    imports: Vec<Import>,
    requests: Vec<CssFileRequest>,
    /// Index into the graph's source list per import, filled as responses
    /// arrive. `None` means the import is still outstanding.
    targets: Vec<Option<usize>>,
}

/// Caller-driven loading with independent, monotonically increasing request IDs.
/// `NeedFile` lists all outstanding requests; hosts may respond in any order.
///
/// The loader performs no I/O and holds no host resources, so a host that
/// abandons a load may simply drop it from any incomplete state.
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
    /// Creates a loader that has not yet received its root source.
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

    /// Answers one request; equivalent to a one-element [`Self::provide_many`].
    ///
    /// The first call must carry exactly one file and becomes the root source.
    pub fn provide(
        &mut self,
        id: u32,
        name: CssSourceName,
        content: Arc<str>,
    ) -> DesignSystemLoadStep {
        self.provide_many([CssFileInput { id, name, content }])
    }

    /// Answers any subset of the outstanding requests in one step.
    ///
    /// A partially answered batch is still parsed immediately: each supplied
    /// source can expose new requests before earlier reads finish, and
    /// unresolved requests reappear in the next `NeedFile`. An empty batch
    /// after the root simply returns the current outstanding list. Once a step
    /// has completed, later calls report [`DesignSystemLoadError::AlreadyComplete`].
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

    /// Accepts one response batch and decides the next step.
    ///
    /// Requests are validated against the IDs issued before this batch, so a
    /// duplicate within the batch and an ID that is unknown or already answered
    /// are both protocol errors. Each accepted source is parsed on its own and
    /// its imports become new requests. Expansion runs only when nothing is
    /// pending; before that, a resolved subgraph is still traversed so a cycle
    /// is reported without waiting for the remaining reads.
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
        // `u32::MAX` marks the synthetic expanded buffer: it is not a source the
        // host supplied, so no host-visible identity may claim it. Token spans
        // that point at it are mapped below into their original buffers.
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

    /// Registers one supplied buffer, or reuses the earlier parse of the same
    /// name after checking that the content is identical.
    ///
    /// The per-source `parse_theme` call is a validation pass: it rejects
    /// unterminated blocks and invalid CSS for this file before the expensive
    /// expansion, and its result is discarded because a source cannot be fully
    /// interpreted until its imports are inlined. Import parsing is recorded
    /// here; request IDs are allocated in authored import order, which is what
    /// makes candidate ordering independent of host response order.
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
    ///
    /// Sources are numbered by acceptance order, not by dependency order, so
    /// this walk is what makes expansion independent of response order. It also
    /// reports a cycle as soon as the resolved subgraph contains one, even
    /// while unrelated reads are still outstanding. A source must be reachable
    /// from the root; the traversal starts at index 0 and errors if the graph
    /// is empty.
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

    /// Inlines every source into one buffer in authored import order.
    ///
    /// `order` is the postorder from [`Self::source_order`], so a source is
    /// expanded only after its imports and can reuse their cached expansion.
    /// Returns only the root's expansion.
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
        let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
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
