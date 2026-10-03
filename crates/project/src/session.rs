//! Mutable host orchestration; snapshots and in-flight request clones stay immutable.
//!
//! A session is an actor: all mutation happens behind an exclusive borrow, and a
//! request handler takes a [`ProjectSession::snapshot`] clone before doing work so it
//! observes one revision for its whole duration.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use stillwind_compiler::{
    CompiledModule, CompilerOptions, GeneratedSymbol, IdentifierPool, Identifiers,
    ModuleArtifactKey, ModuleCompiler, StyleResolver, StylesheetRenderer, check_style,
    is_style_analysis_lint,
};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemSnapshot};
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, DiagnosticBatch, filter_lint_findings,
};
use stillwind_ir::ModuleAnalysis;
use stillwind_source::{FileId, ModuleId, SourceRegistry, SourceRegistryError, Span};

use crate::ReservedNames;
use crate::{DependencyIndex, ProjectStylesheet};

/// Why a design-system reload could not be applied.
#[derive(Debug, thiserror::Error)]
pub enum SessionReloadError {
    /// The new stylesheet configuration failed to load.
    #[error("{0}")]
    Load(
        /// Failure reported by the replacement design-system loader.
        #[from]
        DesignSystemLoadError,
    ),
    /// The retained or rebuilt stylesheet could not be serialized.
    #[error("{0}")]
    Stylesheet(
        /// Failure rendering the project stylesheet for the reload.
        #[from]
        CompileDiagnostic,
    ),
}

/// Whether this session tolerates a failed reload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMode {
    /// A one-shot build. A failed load invalidates artifacts so the host fails the
    /// build instead of emitting stale output.
    Build,
    /// A watch session. A failed load keeps the last known good revision active and
    /// reports diagnostics for the attempted one.
    Watch,
}

/// Result of updating one producer.
#[derive(Clone, Debug, Default)]
pub struct ModuleUpdate {
    /// Whether a successful update published different source transforms.
    pub transformed: bool,
    /// Errors from this update. Non-empty means nothing was published.
    pub diagnostics: Vec<CompileError>,
}

/// What changed after a design-system reload.
#[derive(Clone, Debug, Default)]
pub struct SessionReload {
    /// Whether the assembled project CSS differs from the previous revision.
    pub css_changed: bool,
    /// Modules whose code transforms changed, empty when the reload was rejected.
    pub transformed_modules: BTreeSet<ModuleId>,
    /// Recompilation failures per module.
    pub diagnostics: BTreeMap<ModuleId, Vec<CompileError>>,
}

/// Exclusive mutation is the actor boundary. A host can put this in a mutex; request
/// handlers pin a clone obtained from `snapshot` before doing work.
#[derive(Clone)]
pub struct ProjectSession {
    /// Script source revisions; CSS revisions live in the active design-system snapshot.
    sources: SourceRegistry,
    /// Latest attempted diagnostic batch for each module, independent of active output.
    reported: BTreeMap<ModuleId, DiagnosticBatch>,
    /// Active immutable design-system revision used for compilation.
    current: DesignSystemSnapshot,
    /// Compiler configuration shared by all artifacts in this session.
    options: CompilerOptions,
    /// Naming inputs shared by all artifacts in this session.
    identifiers: Identifiers,
    /// Names the design system's authored CSS already uses, per CSS namespace.
    reserved: ReservedNames,
    /// Names a producer's own CSS uses, keyed by module identity.
    authored: BTreeMap<ModuleId, ReservedNames>,
    /// Whether a failed reload invalidates output or retains the last successful revision.
    mode: SessionMode,
    /// Retained frontend analyses keyed by module identity, used for recompilation.
    analyses: BTreeMap<ModuleId, Arc<ModuleAnalysis>>,
    /// Keys computed before retained analyses become immutable.
    analysis_keys: BTreeMap<ModuleId, Arc<ModuleArtifactKey>>,
    /// Published compiled modules keyed by module identity.
    artifacts: BTreeMap<ModuleId, Arc<CompiledModule>>,
    /// Shared stylesheet assembled from published artifacts.
    stylesheet: ProjectStylesheet,
    /// Reverse memberships derived on demand from this revision's published artifacts.
    dependencies: Arc<OnceLock<DependencyIndex>>,
    /// Most recent rejected load, or None when the active configuration loaded successfully.
    load_error: Option<DesignSystemLoadError>,
}

impl ProjectSession {
    /// Starts a session in the source ID scope used to load its CSS snapshot.
    ///
    /// The naming inputs are fixed for the session's lifetime: a host that changes
    /// generators starts a new session and pairs the change with a new
    /// [`CompilerOptions::naming_revision`], so cached artifacts are not reused across
    /// naming changes.
    pub fn new(
        snapshot: DesignSystemSnapshot,
        sources: &SourceRegistry,
        mode: SessionMode,
        options: CompilerOptions,
        identifiers: Identifiers,
    ) -> Self {
        for source in &snapshot.sources().sources {
            sources.reserve_after(source.source.id());
        }
        let sources = sources.fork_empty();
        let mut reserved = ReservedNames::default();
        reserved.collect(&snapshot.theme().custom_css);
        for source in &snapshot.sources().sources {
            reserved.collect(source.source.text());
        }
        // Theme tokens are declared properties of the emitted theme layer even though a
        // `@theme` block is not a declaration block in the authored text.
        reserved.custom_properties.extend(
            snapshot
                .theme()
                .tokens
                .iter()
                .map(|token| token.name.value.clone()),
        );
        Self {
            sources,
            reported: BTreeMap::new(),
            current: snapshot,
            options,
            identifiers,
            reserved,
            authored: BTreeMap::new(),
            mode,
            analyses: BTreeMap::new(),
            analysis_keys: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            stylesheet: ProjectStylesheet::default(),
            dependencies: Arc::default(),
            load_error: None,
        }
    }

    /// Script source registry issuing the file IDs used by this session's analyses.
    pub fn sources(&self) -> &SourceRegistry {
        &self.sources
    }

    /// Registers input before analysis so spans and public document identity share one source.
    pub fn sources_mut(&mut self) -> &mut SourceRegistry {
        &mut self.sources
    }

    /// Builds the diagnostic lookup from script revisions and the active CSS snapshot.
    ///
    /// # Errors
    ///
    /// Returns a source registry error if a CSS source conflicts with a retained script source.
    pub fn diagnostic_sources(&self) -> Result<SourceRegistry, SourceRegistryError> {
        self.diagnostic_sources_for(&self.current)
    }

    /// Builds the diagnostic lookup for a CSS reload attempt.
    ///
    /// # Errors
    ///
    /// Returns a source registry error if a CSS source conflicts with a retained script source.
    pub fn diagnostic_sources_for(
        &self,
        snapshot: &DesignSystemSnapshot,
    ) -> Result<SourceRegistry, SourceRegistryError> {
        let mut view = self.sources.clone();
        for source in &snapshot.sources().sources {
            if source.source.uri().is_some() {
                view.retain(source.source.clone())?;
            }
        }
        Ok(view)
    }

    /// Retains source revisions from a rejected attempt for its diagnostic batch.
    pub fn retain_attempted_sources(&mut self, attempted: &Self) {
        self.sources = attempted.sources.clone();
    }

    /// Publishes the latest attempted reports while retaining active watch artifacts.
    pub fn retain_attempted_diagnostics(&mut self, attempted: &Self) {
        self.reported = attempted.reported.clone();
    }

    /// Returns the last reported batch for each retained module.
    pub fn diagnostic_batches(&self) -> &BTreeMap<ModuleId, DiagnosticBatch> {
        &self.reported
    }

    /// Replaces one module's diagnostics for its latest attempted input revision.
    pub fn publish_diagnostic_batch(&mut self, module: ModuleId, batch: DiagnosticBatch) {
        self.reported.insert(module, batch);
    }

    /// Retains only script revisions needed for future compilation.
    /// Completed diagnostic batches already contain resolved locations.
    ///
    /// # Errors
    ///
    /// Returns an error if two retained source handles claim the same file ID.
    pub fn prune_sources(&mut self) -> Result<(), SourceRegistryError> {
        let mut retained = self.sources.fork_empty();
        for analysis in self.analyses.values() {
            if analysis.source.uri().is_some() {
                retained.retain(analysis.source.clone())?;
            }
        }
        self.sources = retained;
        Ok(())
    }

    /// Generation options in effect for this revision.
    pub fn options(&self) -> &CompilerOptions {
        &self.options
    }

    /// Naming inputs used by every compilation in this session.
    pub fn identifiers(&self) -> &Identifiers {
        &self.identifiers
    }

    /// Names the design system's authored CSS already uses.
    ///
    /// A generated name that would collide with one of these is reported, because the
    /// authored declaration or reference already owns the name in that namespace.
    /// The set covers the accepted design-system sources and the authored CSS the theme
    /// keeps outside its definition blocks. CSS a producer authors itself is not part of the
    /// design system, so the host supplies it through [`Self::reserve_authored_css`].
    pub fn reserved_names(&self) -> &ReservedNames {
        &self.reserved
    }

    /// Reserves the names one producer's own CSS uses, replacing its previous ones.
    ///
    /// A producer's CSS is not part of the design system, so only the host can supply its text;
    /// the session scans it exactly like the design system's CSS. Names authored here collide with
    /// generated ones like any other authored name.
    ///
    /// Returns the modules whose published artifacts now collide, each with the errors a
    /// recompilation would report; an empty result means every published artifact is still valid.
    pub fn reserve_authored_css(
        &mut self,
        module: &ModuleId,
        css: &str,
    ) -> BTreeMap<ModuleId, Vec<CompileError>> {
        let mut names = ReservedNames::default();
        names.collect(css);
        self.authored.insert(module.clone(), names);
        self.recheck_artifacts()
    }

    /// Forgets one producer's own CSS, for example after the producer is removed.
    ///
    /// Returns the modules whose published artifacts now collide, like
    /// [`Self::reserve_authored_css`] does.
    pub fn forget_authored_css(
        &mut self,
        module: &ModuleId,
    ) -> BTreeMap<ModuleId, Vec<CompileError>> {
        self.authored.remove(module);
        self.recheck_artifacts()
    }

    /// Pins the current immutable design-system revision. Cheap to clone.
    pub fn snapshot(&self) -> DesignSystemSnapshot {
        self.current.clone()
    }

    /// The assembled module stylesheet, including rules from failed updates that watch
    /// mode retained.
    pub fn stylesheet(&self) -> &ProjectStylesheet {
        &self.stylesheet
    }

    /// Published artifacts by module. A module absent here has no current CSS or
    /// transforms, even if an analysis for it is retained.
    pub fn artifacts(&self) -> &BTreeMap<ModuleId, Arc<CompiledModule>> {
        &self.artifacts
    }

    /// Returns a published artifact only when it matches the attempted analysis.
    pub fn current_artifact(&self, module: &ModuleId) -> Option<&CompiledModule> {
        let analysis = self.analyses.get(module)?;
        if !analysis.diagnostics.is_empty() || !analysis.integration_diagnostics.is_empty() {
            return None;
        }
        let key = self.analysis_keys.get(module)?;
        let artifact = self.artifacts.get(module)?;
        (artifact.key == **key).then_some(artifact.as_ref())
    }

    /// Analyses by module, including ones whose compilation failed.
    pub fn analyses(&self) -> &BTreeMap<ModuleId, Arc<ModuleAnalysis>> {
        &self.analyses
    }

    /// Reverse index from design-system definitions and assets to the modules that use
    /// them.
    pub fn dependencies(&self) -> &DependencyIndex {
        self.dependencies.get_or_init(|| {
            let mut index = DependencyIndex::default();
            for (module, artifact) in &self.artifacts {
                index.replace(module, None, &artifact.dependencies);
            }
            index
        })
    }

    /// The most recent design-system load failure, cleared by a success.
    pub fn load_error(&self) -> Option<&DesignSystemLoadError> {
        self.load_error.as_ref()
    }
    /// Renders the complete stylesheet for the current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if stylesheet serialization fails, or when a name the render generated
    /// collides with another name in its CSS namespace.
    pub fn css(&self) -> Result<String, CompileDiagnostic> {
        let renderer = StylesheetRenderer::new(&self.current, &self.options, &self.identifiers);
        let mut stylesheet = self.stylesheet.clone();
        let mut modules = self.artifacts.keys().cloned().collect();
        let order = self
            .stylesheet_order(&mut modules)
            .map_err(|_| CompileDiagnostic::MissingRule)?;
        if modules
            .iter()
            .any(|module| !self.artifacts.contains_key(module))
        {
            return Err(CompileDiagnostic::MissingRule);
        }
        stylesheet.set_order(order);
        let css = stylesheet.serialize(&renderer)?;
        self.check_document_names(&renderer.take_detached_symbols())?;
        Ok(css)
    }

    /// Updates one producer without rendering the project stylesheet.
    ///
    /// Recompilation is skipped when the analysis key already matches the published
    /// artifact, which makes repeated updates during a watch session cheap; the
    /// analysis is still replaced so later queries see the newest one. A module whose
    /// artifact key changed is recompiled. Previous contributions are replaced only
    /// after successful compilation; build-mode failure removes the old artifact.
    ///
    /// Generated identifiers are checked against every other module's symbols, not only
    /// this module's, because all artifacts share one document. A collision is reported
    /// as `CompileDiagnostic::GeneratedIdentifierCollision` and prevents publication.
    /// Each CSS namespace is checked on its own, so one spelling may appear in two
    /// pools without a conflict while two identities inside one pool may not share a
    /// spelling.
    ///
    /// `analysis` must identify the producer and retain source coordinates for its
    /// inputs. Returns whether generated transforms changed and any compilation errors.
    /// Successful publication also recomputes composition findings in direct and
    /// indirect consumers. On failure, watch mode keeps the last good artifact for
    /// display and build mode removes it. Consumer lint checks use only artifacts
    /// matching current analyses.
    pub fn update_module(&mut self, analysis: ModuleAnalysis) -> ModuleUpdate {
        let backend = ModuleCompiler::new(self.current.clone(), self.options.clone());
        let key = backend.key_for(&analysis);
        self.analysis_keys
            .insert(analysis.module_id.clone(), Arc::new(key.clone()));
        if analysis.diagnostics.is_empty()
            && analysis.integration_diagnostics.is_empty()
            && self
                .artifacts
                .get(&analysis.module_id)
                .is_some_and(|artifact| artifact.key == key)
        {
            self.analyses
                .insert(analysis.module_id.clone(), Arc::new(analysis));
            return ModuleUpdate::default();
        }
        let module_id = analysis.module_id.clone();
        let mut compiled = backend.compile(&analysis, &self.identifiers);
        let mut symbols = self.document_names(Some(&module_id));
        for symbol in &compiled.symbols {
            if let Some(collision) = insert_document_name(&mut symbols, symbol) {
                compiled
                    .diagnostics
                    .push(CompileError::new(collision, analysis.source.full_span()));
            }
        }
        self.analyses.insert(module_id.clone(), Arc::new(analysis));
        let mut update = ModuleUpdate::default();
        if compiled.is_success() {
            if self
                .artifacts
                .get(&module_id)
                .is_none_or(|previous| previous.code_transforms != compiled.code_transforms)
            {
                update.transformed = true;
            }
            self.dependencies = Arc::default();
            self.stylesheet.replace(module_id.clone(), &compiled.css);
            let dependents = self.composition_dependents(&module_id);
            self.artifacts.insert(module_id.clone(), Arc::new(compiled));
            self.refresh_style_lints(std::iter::once(module_id).chain(dependents));
        } else {
            update.diagnostics = compiled.diagnostics;
            let dependents = self.composition_dependents(&module_id);
            if self.mode == SessionMode::Build {
                self.remove_artifact(&module_id);
            }
            self.refresh_style_lints(dependents);
        }
        update
    }

    /// Replaces the attempted analyses after a multi-module transaction fails.
    /// `analyses` contains the producers attempted by that transaction. Watch mode
    /// retains their last good artifacts for display; build mode removes them.
    /// Planning and lint resolution exclude artifacts whose keys no longer match.
    /// The method then recomputes composition findings in all transitive consumers;
    /// the binding host must publish diagnostic batches after this state change.
    pub fn reject_updates(&mut self, analyses: impl IntoIterator<Item = ModuleAnalysis>) {
        let mut dependents = BTreeSet::new();
        let backend = ModuleCompiler::new(self.current.clone(), self.options.clone());
        for analysis in analyses {
            self.analysis_keys.insert(
                analysis.module_id.clone(),
                Arc::new(backend.key_for(&analysis)),
            );
            dependents.extend(self.composition_dependents(&analysis.module_id));
            if self.mode == SessionMode::Build {
                self.remove_artifact(&analysis.module_id);
            }
            self.analyses
                .insert(analysis.module_id.clone(), Arc::new(analysis));
        }
        self.refresh_style_lints(dependents);
    }

    /// Removes `module`'s analysis, artifact, stylesheet contributions, and report.
    /// Returns whether its analysis was registered. Composition findings in all
    /// transitive consumers are recomputed with this producer's records unavailable.
    pub fn remove_module(&mut self, module: &ModuleId) -> bool {
        let removed = self.analyses.remove(module).is_some();
        self.analysis_keys.remove(module);
        self.reported.remove(module);
        let dependents = self.composition_dependents(module);
        self.remove_artifact(module);
        self.refresh_style_lints(dependents);
        removed
    }

    /// Replaces composition-dependent findings for the requested module identities.
    /// The caller must first publish or retract changed artifacts and analyses.
    /// Only artifacts matching their current analyses enter the resolver. Modules
    /// without a current artifact are skipped; compatible missing bases remain unknown.
    /// Source suppression runs in each consuming module. Other lint findings remain,
    /// and diagnostic batches are left for the binding host to refresh.
    fn refresh_style_lints(&mut self, modules: impl IntoIterator<Item = ModuleId>) {
        let modules: BTreeSet<_> = modules.into_iter().collect();
        let mut required = modules.clone();
        let mut pending = modules.clone();
        while let Some(module) = pending.pop_first() {
            if let Some(artifact) = self.current_artifact(&module) {
                for edge in &artifact.composition {
                    if required.insert(edge.base.module_id.clone()) {
                        pending.insert(edge.base.module_id.clone());
                    }
                }
            }
        }
        let mut resolver = StyleResolver::new(
            required
                .iter()
                .filter_map(|module| self.current_artifact(module))
                .flat_map(|artifact| &artifact.style_effects),
        );
        let updates: Vec<_> = modules
            .into_iter()
            .filter_map(|module| {
                let source = &self.analyses.get(&module)?.source;
                let artifact = self.current_artifact(&module)?;
                let mut findings = artifact
                    .style_effects
                    .iter()
                    .filter_map(|effects| resolver.resolve(effects.identity()))
                    .flat_map(|style| check_style(&style))
                    .collect::<Vec<_>>();
                filter_lint_findings(source, &mut findings);
                Some((module, findings))
            })
            .collect();
        for (module, findings) in updates {
            if let Some(artifact) = self.artifacts.get_mut(&module) {
                let artifact = Arc::make_mut(artifact);
                artifact
                    .lint_findings
                    .retain(|finding| !is_style_analysis_lint(finding.kind()));
                artifact.lint_findings.extend(findings);
            }
        }
    }

    /// Returns every published module that directly or indirectly consumes `module`.
    /// Traversal uses retained artifact edges, including last good watch artifacts.
    /// The starting module is omitted, each consumer occurs once, and cycles terminate.
    /// Ordering follows breadth-first traversal with module order among each producer's
    /// consumers.
    fn composition_dependents(&self, module: &ModuleId) -> Vec<ModuleId> {
        let mut pending = vec![module.clone()];
        let mut seen = BTreeSet::from([module.clone()]);
        let mut dependents = Vec::new();
        while let Some(base) = pending.pop() {
            for (id, artifact) in &self.artifacts {
                if artifact
                    .composition
                    .iter()
                    .any(|edge| edge.base.module_id == base)
                    && seen.insert(id.clone())
                {
                    dependents.push(id.clone());
                    pending.push(id.clone());
                }
            }
        }
        dependents
    }

    fn remove_artifact(&mut self, module: &ModuleId) {
        if self.artifacts.remove(module).is_some() {
            self.stylesheet.remove(module);
            self.dependencies = Arc::default();
        }
    }

    /// Publishes an entirely rebuilt revision under the host's exclusive actor turn.
    ///
    /// Every retained analysis is recompiled against the new snapshot before anything
    /// is published, because definition changes can invalidate output that the source
    /// revision did not touch.
    ///
    /// # Errors
    ///
    /// Returns the attempted load failure. Watch mode retains all active output; build
    /// mode invalidates artifacts and requires the host to fail the build.
    pub fn reload(
        &mut self,
        result: Result<DesignSystemSnapshot, DesignSystemLoadError>,
        options: CompilerOptions,
    ) -> Result<SessionReload, SessionReloadError> {
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.load_error = Some(error.clone());
                if self.mode == SessionMode::Build {
                    self.artifacts.clear();
                    self.stylesheet = ProjectStylesheet::default();
                    self.dependencies = Arc::default();
                }
                return Err(error.into());
            }
        };
        let old_css = self.css();
        let mut next = Self::new(
            snapshot,
            &self.sources,
            self.mode,
            options,
            self.identifiers.clone(),
        );
        next.sources = self.sources.clone();
        next.reported = self.reported.clone();
        next.authored = self.authored.clone();
        if next.current.fingerprints().content == self.current.fingerprints().content
            && next.options == self.options
        {
            next.artifacts = self.artifacts.clone();
            next.stylesheet = self.stylesheet.clone();
            next.dependencies = self.dependencies.clone();
        }
        let mut update = SessionReload::default();
        for (id, analysis) in &self.analyses {
            let changed = next.update_module(analysis.as_ref().clone());
            if !changed.diagnostics.is_empty() {
                update.diagnostics.insert(id.clone(), changed.diagnostics);
            }
            if next.artifacts.get(id).map(|value| &value.code_transforms)
                != self.artifacts.get(id).map(|value| &value.code_transforms)
            {
                update.transformed_modules.insert(id.clone());
            }
        }
        if self.mode == SessionMode::Watch && !update.diagnostics.is_empty() {
            // A revision is published as one coherent set, never a mixture of new
            // registries and stale successful artifacts.
            update.transformed_modules.clear();
            return Ok(update);
        }
        let next_css = next.css()?;
        update.css_changed = !old_css.is_ok_and(|previous| previous == next_css);
        *self = next;
        Ok(update)
    }

    /// The names the document already uses, by CSS namespace.
    ///
    /// `except` names one module whose symbols are left out, so it can be checked against the rest
    /// of the document. An authored name already owns its namespace, so a generated name equal to
    /// one is reported even when no producer generated it.
    fn document_names(&self, except: Option<&ModuleId>) -> BTreeMap<(IdentifierPool, &str), &str> {
        let mut names: BTreeMap<_, _> = self
            .artifacts
            .iter()
            .filter(|(id, _)| except != Some(*id))
            .flat_map(|(_, artifact)| artifact.symbols.iter())
            .map(|symbol| {
                (
                    (symbol.pool, symbol.name.as_str()),
                    symbol.identity.as_str(),
                )
            })
            .collect();
        insert_reserved(&mut names, &self.reserved);
        for authored in self.authored.values() {
            insert_reserved(&mut names, authored);
        }
        names
    }

    /// Checks every published artifact against the document's names, in module order.
    fn recheck_artifacts(&self) -> BTreeMap<ModuleId, Vec<CompileError>> {
        let mut names = self.document_names(None);
        let mut reported = BTreeMap::new();
        for (id, artifact) in &self.artifacts {
            let mut diagnostics = Vec::new();
            for symbol in &artifact.symbols {
                if let Some(collision) = insert_document_name(&mut names, symbol) {
                    diagnostics.push(CompileError::new(collision, self.module_span(id)));
                }
            }
            if !diagnostics.is_empty() {
                reported.insert(id.clone(), diagnostics);
            }
        }
        reported
    }

    /// The span one module's analysis covers, or an empty span when it is unknown.
    fn module_span(&self, module: &ModuleId) -> Span {
        self.analyses.get(module).map_or_else(
            || Span::new(FileId(0), 0, 0),
            |analysis| analysis.source.full_span(),
        )
    }

    /// Reports the first collision among names a stylesheet render generated, if any.
    ///
    /// Module artifacts are checked when they are published; this covers the names no
    /// declaration reports, such as conditional helpers and anonymous layers.
    pub(crate) fn check_document_names(
        &self,
        symbols: &[GeneratedSymbol],
    ) -> Result<(), CompileDiagnostic> {
        let mut names = self.document_names(None);
        for symbol in symbols {
            if let Some(collision) = insert_document_name(&mut names, symbol) {
                return Err(collision);
            }
        }
        Ok(())
    }
}

/// Records every name one authored stylesheet already uses.
///
/// An authored name owns its namespace before anything is generated, so it is recorded
/// with an identity no generated name can have.
fn insert_reserved<'a>(
    names: &mut BTreeMap<(IdentifierPool, &'a str), &'a str>,
    reserved: &'a ReservedNames,
) {
    for (pool, names_in_pool) in [
        (IdentifierPool::CustomProperty, &reserved.custom_properties),
        (IdentifierPool::LayerName, &reserved.layers),
        (IdentifierPool::KeyframesName, &reserved.keyframes),
        (IdentifierPool::ClassToken, &reserved.classes),
        (IdentifierPool::AnchorName, &reserved.anchor_names),
        (IdentifierPool::ContainerName, &reserved.container_names),
        (
            IdentifierPool::ViewTransitionClass,
            &reserved.view_transition_classes,
        ),
    ] {
        for name in names_in_pool {
            names.insert((pool, name.as_str()), "<authored CSS>");
        }
    }
}

/// Records one generated name, reporting it when the namespace already holds another
/// identity.
fn insert_document_name<'a>(
    names: &mut BTreeMap<(IdentifierPool, &'a str), &'a str>,
    symbol: &'a GeneratedSymbol,
) -> Option<CompileDiagnostic> {
    names
        .insert(
            (symbol.pool, symbol.name.as_str()),
            symbol.identity.as_str(),
        )
        .filter(|identity| *identity != symbol.identity)
        .map(|_| CompileDiagnostic::GeneratedIdentifierCollision {
            name: symbol.name.clone().into(),
        })
}
