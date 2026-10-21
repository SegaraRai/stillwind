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
//! known name again is allowed only with the same URI and byte-identical content.
//! Failures the loader can decide from responses (unknown IDs, CSS that cannot be
//! divided into statements, conflicting content, exhausted IDs) become
//! [`Complete(Err(..))`](DesignSystemLoadStep::Complete); a host that cannot read a
//! requested source drops the loader instead, so no host failure is ever reported as a
//! loader error. An invalid import or directive, and an import that closes a cycle, are
//! skipped and recorded in [`CssSourceGraph::skipped`] instead.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::{
    CssSourceName, FileId, SourceError, SourceFile, SourceRegistry, SourceRegistryError, SourceUri,
    Span,
};

use super::expansion::ExpandedSource;
use super::imports::{Import, scan};
use super::protocol::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssSource, CssSourceEdge, CssSourceGraph,
    DesignSystemLoadStep,
};
use crate::skipped::locate;
use crate::theme::parse_theme_builder;
use crate::{DesignSystemOptions, DesignSystemSnapshot, SkipReason, SkippedCss};

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
    Source(
        /// Failure storing a supplied source in the byte-coordinate space.
        #[from]
        SourceError,
    ),
    /// Registration of a host-addressable CSS document failed.
    #[error("{0}")]
    Registry(#[from] SourceRegistryError),
    /// The monotonic request counter reached `u32::MAX`.
    #[error("CSS request IDs exhausted")]
    RequestIdExhausted,
    /// A response arrived after the load had already completed.
    #[error("provide called after completion")]
    AlreadyComplete,
    /// The root source was answered as unresolved.
    #[error("the root CSS source cannot be unresolved")]
    UnresolvedRoot,
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
    /// A known name was supplied again with different document identity or content.
    #[error("conflicting source for {}", .name.as_str())]
    ConflictingSource {
        /// Name that was supplied twice.
        name: CssSourceName,
    },
    /// A source could not be compiled.
    #[error(transparent)]
    Compilation(
        /// Source-located cause; labels use accepted-source IDs, never the expanded buffer ID.
        #[from]
        CompileError,
    ),
    /// The resolved prefix configuration is invalid.
    #[error("{0}")]
    InvalidOptions(
        /// Validation failure for the resolved prefix configuration.
        #[from]
        CompileDiagnostic,
    ),
}

/// One accepted source: its imports, the requests issued for them, and the
/// resolved target index per import slot.
struct Node {
    /// Parsed imports in authored order, indexed in parallel with requests and targets.
    imports: Vec<Import>,
    /// Host requests in import order; each entry corresponds to the same index in
    /// imports.
    requests: Vec<CssFileRequest>,
    /// Index into the graph's source list per import, filled as responses
    /// arrive. `None` means the import is still outstanding.
    targets: Vec<Option<usize>>,
    /// Byte ranges of the `@config` statements and the skipped import statements in
    /// source order, which expansion removes. They never overlap an import's range.
    removed: Vec<Range<usize>>,
}

/// One import of one accepted source: (source index, import index).
type ImportSlot = (usize, usize);

/// Caller-driven loading with independent, monotonically increasing request IDs.
/// `NeedFile` lists all outstanding requests; hosts may respond in any order.
///
/// The loader performs no I/O and holds no host resources, so a host that
/// abandons a load may simply drop it from any incomplete state.
pub struct DesignSystemLoader {
    /// Original root revision retained by the standalone source entry point.
    supplied_root: Option<Arc<SourceFile>>,
    /// Issued identities for host-addressable CSS source revisions.
    registry: SourceRegistry,
    /// Requested prefix configuration validated when loading completes.
    options: DesignSystemOptions,
    /// Sources and import edges retained for the current load.
    graph: CssSourceGraph,
    /// Supplied source name to index in the graph's source list.
    known: BTreeMap<CssSourceName, usize>,
    /// Per-source import state in the same order as the graph's sources.
    nodes: Vec<Node>,
    /// Outstanding request id to (source index, import index); indices address nodes
    /// and its import arrays.
    pending: BTreeMap<u32, (usize, usize)>,
    /// Next monotonically allocated request id; allocation fails rather than wrapping.
    next_id: u32,
    /// Whether completion or failure has ended the protocol; further responses are
    /// rejected.
    terminal: bool,
    /// Specifier of the `@plugin` request that follows the root's text, until the root
    /// is accepted.
    trailing_plugin: Option<LeanStr>,
    /// Import statements that the per-source scan skipped, and imports that the host
    /// answered as unresolved, in acceptance order.
    skipped: Vec<SkippedCss>,
    /// Imports that the host answered as unresolved.
    unresolved: Vec<ImportSlot>,
}

impl DesignSystemLoader {
    /// Returns source revisions accepted so far, including after a failed load.
    pub fn sources(&self) -> &SourceRegistry {
        &self.registry
    }

    /// Returns the first accepted source used for load-wide errors.
    pub fn root_source(&self) -> Option<&Arc<SourceFile>> {
        self.graph
            .sources
            .first()
            .map(|source| &source.source)
            .or_else(|| self.registry.first())
    }

    /// Creates a loader whose CSS revisions share file IDs with a project session.
    pub fn new(options: DesignSystemOptions, registry: SourceRegistry) -> Self {
        Self {
            supplied_root: None,
            registry,
            options,
            graph: CssSourceGraph::default(),
            known: BTreeMap::new(),
            nodes: Vec::new(),
            pending: BTreeMap::new(),
            next_id: 1,
            terminal: false,
            trailing_plugin: None,
            skipped: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    /// Adds a `@plugin` request without options after the root's text. The answer
    /// therefore follows every authored rule in the expansion, and its definitions are
    /// plugin definitions. Hosts use it for project settings that no stylesheet names.
    ///
    /// `specifier` must differ from every authored `@plugin` specifier that the host
    /// answers differently. Decoded CSS never contains NUL, so a specifier with NUL
    /// cannot collide.
    ///
    /// Returns false, without changing the loader, after the root has been supplied.
    pub fn request_trailing_plugin(&mut self, specifier: LeanStr) -> bool {
        if !self.nodes.is_empty() {
            return false;
        }
        self.trailing_plugin = Some(specifier);
        true
    }

    /// Starts standalone loading with the caller's root file identity.
    /// The first response must contain this root's text and metadata.
    pub(crate) fn with_source(options: DesignSystemOptions, source: Arc<SourceFile>) -> Self {
        let registry = SourceRegistry::default();
        registry.reserve_after(source.id());
        let mut loader = Self::new(options, registry);
        loader.supplied_root = Some(source);
        loader
    }

    /// Answers one request; equivalent to a one-element [`Self::provide_many`].
    ///
    /// The first call must carry exactly one file and becomes the root source. `None`
    /// for `uri` identifies an internal buffer without a reportable document. The
    /// source is not marked for stylesheet lint. [`Self::provide_many`] can mark it.
    pub fn provide(
        &mut self,
        id: u32,
        name: CssSourceName,
        uri: Option<SourceUri>,
        source_map_source: Option<Arc<str>>,
        content: Arc<str>,
    ) -> DesignSystemLoadStep {
        self.provide_many([CssFileInput {
            id,
            name,
            uri,
            source_map_source,
            content,
            lint: false,
            unresolved: false,
        }])
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
    /// Requests are validated against the IDs issued before this batch, so a duplicate
    /// within the batch and an ID that is unknown or already answered are both protocol
    /// errors. Each accepted source is parsed on its own and its imports become new
    /// requests. Expansion runs only when nothing is pending, and an import that closes
    /// a cycle is then skipped.
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
            if file.unresolved {
                let (source, import) = parent.ok_or(DesignSystemLoadError::UnresolvedRoot)?;
                self.skip_unresolved(source, import)?;
                continue;
            }
            let index = self.accept_source(
                file.name.clone(),
                file.uri,
                file.source_map_source,
                file.content,
            )?;
            if let Some(source) = self.graph.sources.get_mut(index) {
                source.lint |= file.lint;
            }
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
        self.complete()
    }

    /// Expands the complete graph and parses it into the final snapshot. Imports that
    /// close a cycle are skipped and recorded first.
    fn complete(&mut self) -> Result<DesignSystemLoadStep, DesignSystemLoadError> {
        let (order, cycles) = self.source_order()?;
        for &(source, import) in &cycles {
            let node = self
                .nodes
                .get(source)
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            let (import, request) = node
                .imports
                .get(import)
                .zip(node.requests.get(import))
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            let file = self
                .graph
                .sources
                .get(source)
                .ok_or(DesignSystemLoadError::InvalidGraph)?
                .source
                .id();
            self.skipped.extend(locate(
                file,
                vec![(
                    import.start..import.end,
                    SkipReason::ImportCycle {
                        specifier: request.specifier.clone(),
                    },
                )],
            ));
        }
        let mut skipped_imports = cycles;
        skipped_imports.extend_from_slice(&self.unresolved);
        let output = self.expand(&order, &skipped_imports)?;
        let root = self
            .graph
            .sources
            .first()
            .ok_or(DesignSystemLoadError::InvalidGraph)?;
        let name = root.name.clone();
        // `u32::MAX` marks the synthetic expanded buffer: it is not a source the
        // host supplied, so no host-visible identity may claim it. Token spans
        // that point at it are mapped below into their original buffers.
        let source = SourceFile::new(FileId(u32::MAX), None, None, output.text.as_str())?;
        let mut theme =
            parse_theme_builder(&source, &name, &output.plugin_ranges).map_err(|mut error| {
                // Whole-expansion failures use the root when no contiguous original span exists.
                for label in &mut error.labels {
                    label.span = output
                        .original_span(label.span)
                        .unwrap_or_else(|| root.source.full_span());
                }
                error
            })?;
        theme.utility_order = self.options.utility_order;
        theme.target = self.options.target.clone();
        theme.map_template_origins(|span| output.original_span(span));
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
        let mut skipped = std::mem::take(&mut self.skipped);
        for skip in locate(source.id(), std::mem::take(&mut theme.skipped)) {
            // A statement that wrappers or joined sources produced keeps the start of
            // its range in the authored source.
            let span = output.original_span(skip.span).or_else(|| {
                output.original_span(Span::new(
                    skip.span.file,
                    skip.span.start.0,
                    skip.span.start.0,
                ))
            });
            if let Some(span) = span {
                skipped.push(SkippedCss { span, ..skip });
            }
        }
        skipped.sort_by_key(|skip| (skip.span.file, skip.span.start, skip.span.end));
        skipped.dedup();
        self.graph.skipped = skipped;
        self.graph.mappings = output.mappings;
        self.graph.resources = output.resources;
        let snapshot = DesignSystemSnapshot::from_loaded_theme(
            theme.finish(),
            self.options.clone(),
            std::mem::take(&mut self.graph),
        )
        .map_err(DesignSystemLoadError::InvalidOptions)?;
        self.terminal = true;
        Ok(DesignSystemLoadStep::Complete(Ok(snapshot)))
    }

    /// Records that the host could not resolve or read import `import` of source
    /// `source`, so expansion leaves the import out.
    fn skip_unresolved(
        &mut self,
        source: usize,
        import: usize,
    ) -> Result<(), DesignSystemLoadError> {
        let node = self
            .nodes
            .get(source)
            .ok_or(DesignSystemLoadError::InvalidGraph)?;
        let (statement, request) = node
            .imports
            .get(import)
            .zip(node.requests.get(import))
            .ok_or(DesignSystemLoadError::InvalidGraph)?;
        let file = self
            .graph
            .sources
            .get(source)
            .ok_or(DesignSystemLoadError::InvalidGraph)?
            .source
            .id();
        self.skipped.extend(locate(
            file,
            vec![(
                statement.start..statement.end,
                SkipReason::UnresolvedImport {
                    specifier: request.specifier.clone(),
                },
            )],
        ));
        self.unresolved.push((source, import));
        Ok(())
    }

    /// Registers one supplied buffer, or reuses the earlier parse of the same
    /// name after checking that the content is identical.
    ///
    /// The per-source parse is a validation pass: it rejects
    /// unterminated blocks and invalid CSS for this file before the expensive
    /// expansion, and its result is discarded because a source cannot be fully
    /// interpreted until its imports are inlined. Import parsing is recorded
    /// here; request IDs are allocated in authored import order, which is what
    /// makes candidate ordering independent of host response order.
    fn accept_source(
        &mut self,
        name: CssSourceName,
        uri: Option<SourceUri>,
        source_map_source: Option<Arc<str>>,
        content: Arc<str>,
    ) -> Result<usize, DesignSystemLoadError> {
        if let Some(&index) = self.known.get(&name) {
            let previous = &self
                .graph
                .sources
                .get(index)
                .ok_or(DesignSystemLoadError::InvalidGraph)?
                .source;
            if previous.text() != content.as_ref()
                || previous.uri() != uri.as_ref()
                || previous.source_map_source() != source_map_source.as_deref()
            {
                return Err(DesignSystemLoadError::ConflictingSource { name: name.clone() });
            }
            return Ok(index);
        }
        let index = self.nodes.len();
        let source = if let Some(source) = self.supplied_root.take() {
            self.registry.retain(source.clone())?;
            source
        } else {
            self.registry.register(uri, source_map_source, content)?
        };
        let mut scan =
            scan(source.text()).map_err(|kind| CompileError::new(kind, source.full_span()))?;
        let removed = scan.removed();
        self.skipped
            .extend(locate(source.id(), std::mem::take(&mut scan.skipped)));
        let mut imports = scan.imports;
        // Expansion leaves out the removed statements, so the validation pass does not
        // read them either.
        let validated = if removed.is_empty() {
            source.clone()
        } else {
            let mut text = source.text().as_bytes().to_vec();
            for range in &removed {
                for byte in &mut text[range.clone()] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }
            }
            Arc::new(SourceFile::new(
                source.id(),
                None,
                None,
                String::from_utf8_lossy(&text).as_ref(),
            )?)
        };
        parse_theme_builder(&validated, &name, &[])?;
        if index == 0
            && let Some(specifier) = self.trailing_plugin.take()
        {
            let end = source.text().len();
            imports.push(Import {
                start: end,
                end,
                specifier,
                wrappers: Vec::new(),
                plugin: Some(Vec::new()),
            });
        }
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
                kind: import
                    .plugin
                    .clone()
                    .map_or(CssDependencyKind::Import, |options| {
                        CssDependencyKind::Plugin { options }
                    }),
            });
            self.pending.insert(id, (index, import_index));
        }
        self.nodes.push(Node {
            targets: vec![None; imports.len()],
            imports,
            requests,
            removed,
        });
        self.known.insert(name.clone(), index);
        self.graph.sources.push(CssSource {
            name,
            source,
            lint: false,
        });
        Ok(index)
    }

    /// Postorder traversal using heap frames.
    ///
    /// Sources are numbered by acceptance order, not by dependency order, so this walk
    /// is what makes expansion independent of response order. Returns the order and the
    /// imports, as (source index, import index), that close a cycle; the walk does not
    /// follow them. A source must be reachable from the root; the traversal starts at
    /// index 0 and errors if the graph is empty.
    fn source_order(&self) -> Result<(Vec<usize>, Vec<ImportSlot>), DesignSystemLoadError> {
        let mut state = vec![0; self.nodes.len()];
        *state
            .first_mut()
            .ok_or(DesignSystemLoadError::InvalidGraph)? = 1;
        let mut frames = vec![(0, 0)];
        let mut cycles = Vec::new();
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
                continue;
            };
            let import = *next;
            *next += 1;
            let Some(target) = target else { continue };
            let mark = state
                .get_mut(*target)
                .ok_or(DesignSystemLoadError::InvalidGraph)?;
            if *mark == 1 {
                cycles.push((*index, import));
            } else if *mark == 0 {
                *mark = 1;
                frames.push((*target, 0));
            }
        }
        Ok((order, cycles))
    }

    /// Inlines every source into one buffer in authored import order.
    ///
    /// `order` is the postorder from [`Self::source_order`], so a source is expanded
    /// only after its imports and can reuse their cached expansion. The imports in
    /// `skipped`, as (source index, import index), are left out with their statements.
    /// Returns only the root's expansion.
    fn expand(
        &self,
        order: &[usize],
        skipped: &[ImportSlot],
    ) -> Result<ExpandedSource, DesignSystemLoadError> {
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
            let name = &self.graph.sources[index].name;
            let mut output = ExpandedSource::default();
            let mut cursor = 0;
            let mut removed = node.removed.iter().peekable();
            // Copies the source up to `end`, leaving out the `@config` statements before it.
            let mut copy = |output: &mut ExpandedSource, cursor: usize, end: usize| {
                let mut start = cursor;
                while let Some(range) = removed.next_if(|range| range.end <= end) {
                    output.append_source(source, name, start..range.start)?;
                    start = range.end;
                }
                output.append_source(source, name, start..end)
            };
            for (import_index, (import, target)) in
                node.imports.iter().zip(&node.targets).enumerate()
            {
                copy(&mut output, cursor, import.start)?;
                if skipped.contains(&(index, import_index)) {
                    cursor = import.end;
                    continue;
                }
                let target = target.ok_or(DesignSystemLoadError::InvalidGraph)?;
                let imported = cache
                    .get(&target)
                    .ok_or(DesignSystemLoadError::InvalidGraph)?;
                output.append_import(imported, &import.wrappers, import.plugin.is_some())?;
                cursor = import.end;
            }
            copy(&mut output, cursor, source.text().len())?;
            cache.insert(index, output);
        }
        cache.remove(&0).ok_or(DesignSystemLoadError::InvalidGraph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain_loader(depth: usize) -> DesignSystemLoader {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        for index in 0..depth {
            let text = if index + 1 < depth {
                "@import 'next';"
            } else {
                ".x{color:red;}"
            };
            let source = Arc::new(
                SourceFile::new(FileId(u32::try_from(index + 1).unwrap()), None, None, text)
                    .unwrap(),
            );
            let imports = scan(text).unwrap().imports;
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
                removed: Vec::new(),
            });
            loader.graph.sources.push(CssSource {
                name: "node.css".into(),
                source,
                lint: false,
            });
        }
        loader
    }

    #[test]
    fn deep_import_graphs_use_heap_frames() {
        let loader = chain_loader(20_000);
        let (order, cycles) = loader.source_order().unwrap();
        assert_eq!(order, (0..20_000).rev().collect::<Vec<_>>());
        assert_eq!(cycles, []);
        let loader = chain_loader(512);
        let output = loader
            .expand(&loader.source_order().unwrap().0, &[])
            .unwrap();
        assert_eq!(output.text, ".x{color:red;}");
    }

    #[test]
    fn incomplete_expansion_returns_a_graph_error() {
        let mut loader = chain_loader(2);
        loader.nodes[0].targets[0] = None;
        assert!(matches!(
            loader.expand(&loader.source_order().unwrap().0, &[]),
            Err(DesignSystemLoadError::InvalidGraph)
        ));
    }

    #[test]
    fn request_id_exhaustion_is_terminal_and_never_emits_max_or_wraps() {
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        loader.next_id = u32::MAX - 1;
        let DesignSystemLoadStep::NeedFile(requests) =
            loader.provide(0, "root".into(), None, None, "@import 'last';".into())
        else {
            panic!("expected last usable request ID");
        };
        assert_eq!(requests[0].id, u32::MAX - 1);
        assert!(matches!(
            loader.provide(
                u32::MAX - 1,
                "last".into(),
                None,
                None,
                "@import 'overflow';".into()
            ),
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
        let mut loader =
            DesignSystemLoader::new(DesignSystemOptions::default(), SourceRegistry::default());
        loader.next_id = u32::MAX - 1;
        assert!(matches!(
            loader.provide(
                0,
                "root".into(),
                None,
                None,
                "@import 'last'; @import 'overflow';".into()
            ),
            DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::RequestIdExhausted))
        ));
        assert_eq!(loader.next_id, u32::MAX);
    }
}
