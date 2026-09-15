//! Mutable host orchestration; snapshots and in-flight request clones stay immutable.
//!
//! A session is an actor: all mutation happens behind an exclusive borrow, and a
//! request handler takes a [`ProjectSession::snapshot`] clone before doing work so it
//! observes one revision for its whole duration.
//!
//! Module CSS is stored per producer and deduplicated by generated rule key, so two
//! modules emitting the same rule contribute one copy that survives until the last user
//! is removed. Emission order follows the contribution map, which is ordered by module
//! id, while declaration order inside a rule is already fixed by the compiler.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_compiler::{CompiledModule, CompilerOptions, ModuleCompiler};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::ModuleAnalysis;
use stillwind_semantic::ModuleId;

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
        CompileErrorKind,
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
pub struct ProjectSession {
    /// Active immutable design-system revision used for compilation.
    current: DesignSystemSnapshot,
    /// Compiler configuration shared by all artifacts in this session.
    options: CompilerOptions,
    /// Whether a failed reload invalidates output or retains the last successful revision.
    mode: SessionMode,
    /// Retained frontend analyses keyed by module identity, used for recompilation.
    analyses: BTreeMap<ModuleId, ModuleAnalysis>,
    /// Published compiled modules keyed by module identity.
    artifacts: BTreeMap<ModuleId, CompiledModule>,
    /// Shared stylesheet assembled from published artifacts.
    stylesheet: ProjectStylesheet,
    /// Reverse dependency lookup used to select invalidated modules.
    dependencies: DependencyIndex,
    /// Most recent rejected load, or None when the active configuration loaded successfully.
    load_error: Option<DesignSystemLoadError>,
}

impl ProjectSession {
    /// Starts a session from a completed snapshot and its generation options.
    pub fn new(
        snapshot: DesignSystemSnapshot,
        mode: SessionMode,
        options: CompilerOptions,
    ) -> Self {
        Self {
            current: snapshot,
            options,
            mode,
            analyses: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            stylesheet: ProjectStylesheet::default(),
            dependencies: DependencyIndex::default(),
            load_error: None,
        }
    }

    /// Generation options in effect for this revision.
    pub fn options(&self) -> &CompilerOptions {
        &self.options
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
    pub fn artifacts(&self) -> &BTreeMap<ModuleId, CompiledModule> {
        &self.artifacts
    }

    /// Analyses by module, including ones whose compilation failed.
    pub fn analyses(&self) -> &BTreeMap<ModuleId, ModuleAnalysis> {
        &self.analyses
    }

    /// Reverse index from design-system definitions and assets to the modules that use
    /// them.
    pub fn dependencies(&self) -> &DependencyIndex {
        &self.dependencies
    }

    /// The most recent design-system load failure, cleared by a success.
    pub fn load_error(&self) -> Option<&DesignSystemLoadError> {
        self.load_error.as_ref()
    }
    /// Renders the complete stylesheet for the current revision.
    ///
    /// # Errors
    ///
    /// Returns an error if stylesheet serialization fails.
    pub fn css(&self) -> Result<String, CompileErrorKind> {
        self.stylesheet.serialize(&self.current, &self.options)
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
    /// as `SHSYMBOL001` and prevents publication.
    ///
    /// On failure the behaviour follows [`SessionMode`]: watch keeps the last good
    /// artifact, build removes it.
    pub fn update_module(&mut self, analysis: ModuleAnalysis) -> ModuleUpdate {
        let backend = ModuleCompiler::new(self.current.clone(), self.options.clone());
        if analysis.diagnostics.is_empty()
            && analysis.integration_diagnostics.is_empty()
            && self
                .artifacts
                .get(&analysis.module_id)
                .is_some_and(|artifact| artifact.key == backend.key_for(&analysis))
        {
            self.analyses.insert(analysis.module_id.clone(), analysis);
            return ModuleUpdate::default();
        }
        let module_id = analysis.module_id.clone();
        let mut compiled = backend.compile(&analysis);
        let mut symbols: BTreeMap<_, _> = self
            .artifacts
            .iter()
            .filter(|(id, _)| *id != &module_id)
            .flat_map(|(_, artifact)| artifact.symbols.iter())
            .map(|symbol| (symbol.name.as_str(), symbol.identity.as_str()))
            .collect();
        for symbol in &compiled.symbols {
            if symbols
                .insert(&symbol.name, &symbol.identity)
                .is_some_and(|identity| identity != symbol.identity)
            {
                compiled.diagnostics.push(CompileError::new(
                    CompileErrorKind::GeneratedIdentifierCollision {
                        name: symbol.name.clone().into(),
                    },
                    analysis.source.full_span(),
                ));
            }
        }
        self.analyses.insert(module_id.clone(), analysis);
        let mut update = ModuleUpdate::default();
        if compiled.is_success() {
            if self
                .artifacts
                .get(&module_id)
                .is_none_or(|previous| previous.code_transforms != compiled.code_transforms)
            {
                update.transformed = true;
            }
            self.dependencies.replace(
                &module_id,
                self.artifacts
                    .get(&module_id)
                    .map(|previous| &previous.dependencies),
                &compiled.dependencies,
            );
            self.stylesheet.replace(module_id.clone(), &compiled.css);
            self.artifacts.insert(module_id, compiled);
        } else {
            update.diagnostics = compiled.diagnostics;
            if self.mode == SessionMode::Build {
                self.remove_artifact(module_id.as_str());
            }
        }
        update
    }

    /// Removes one producer, returning whether it was registered.
    pub fn remove_module(&mut self, module: &str) -> bool {
        let removed = self.analyses.remove(module).is_some();
        self.remove_artifact(module);
        removed
    }

    fn remove_artifact(&mut self, module: &str) {
        if let Some((id, artifact)) = self.artifacts.remove_entry(module) {
            self.stylesheet.remove(module);
            self.dependencies.remove(&id, &artifact.dependencies);
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
                    self.dependencies = DependencyIndex::default();
                }
                return Err(error.into());
            }
        };
        let old_css = self.css();
        let mut next = Self::new(snapshot, self.mode, options);
        let mut update = SessionReload::default();
        for (id, analysis) in &self.analyses {
            let changed = next.update_module(analysis.clone());
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
}

#[cfg(test)]
mod tests {
    use stillwind_compiler::CompilerOptions;
    use stillwind_design_system::DesignSystemOptions;
    use stillwind_ir::SourceAnalyzer;
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    fn reload_snapshot(css: &str) -> DesignSystemSnapshot {
        DesignSystemSnapshot::from_source(
            &crate::css_fixture::source(FileId(2), "theme", css).unwrap(),
            DesignSystemOptions::default(),
        )
        .unwrap()
    }

    fn reload_module() -> ModuleAnalysis {
        let source = SourceFile::new(
            FileId(1),
            "motion.tsx",
            "import { style } from 'stillwind/react'; const motion = style.animation({ from: 'brand' }); const x = style(motion('both')); ",
        ).unwrap();
        OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(source)), None)
    }

    #[test]
    fn watch_reload_rejects_the_entire_revision_when_recompilation_fails() {
        let mut session = ProjectSession::new(
            reload_snapshot("@utility brand { color:red; }"),
            SessionMode::Watch,
            CompilerOptions::default(),
        );
        assert!(
            session
                .update_module(reload_module())
                .diagnostics
                .is_empty()
        );
        let other = SourceFile::new(
            FileId(3),
            "other.tsx",
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap();
        let analysis =
            OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(other)), None);
        assert!(session.update_module(analysis).diagnostics.is_empty());
        let other_artifact = session.artifacts()["other.tsx"].css.to_css();
        let before_css = session.css().unwrap();
        let revision = session.snapshot().revision();
        let before_artifact = session.artifacts()["motion.tsx"].css.to_css();
        let update = session
            .reload(Ok(reload_snapshot("")), session.options().clone())
            .unwrap();
        assert!(
            update.diagnostics["motion.tsx"]
                .iter()
                .any(|error| matches!(error.kind, CompileErrorKind::UnavailableUtility { .. }))
        );
        assert!(!update.css_changed);
        assert!(update.transformed_modules.is_empty());
        assert_eq!(session.snapshot().revision(), revision);
        assert_eq!(session.css().unwrap(), before_css);
        assert_eq!(
            session.artifacts()["motion.tsx"].css.to_css(),
            before_artifact
        );
        assert_eq!(session.artifacts().len(), 2);
        assert_eq!(
            session.artifacts()["other.tsx"].css.to_css(),
            other_artifact
        );
        assert!(session.dependencies().utilities.contains_key("brand"));
        assert!(
            session
                .reload(
                    Ok(reload_snapshot("@utility brand { color:blue; }")),
                    session.options().clone()
                )
                .unwrap()
                .css_changed
        );
    }

    #[test]
    fn build_load_failure_clears_artifacts_and_dependency_indexes() {
        let mut session = ProjectSession::new(
            reload_snapshot("@utility brand { color:red; }"),
            SessionMode::Build,
            CompilerOptions::default(),
        );
        assert!(
            session
                .update_module(reload_module())
                .diagnostics
                .is_empty()
        );
        assert!(!session.artifacts().is_empty());
        assert!(!session.dependencies().utilities.is_empty());
        assert!(
            session
                .reload(
                    Err(DesignSystemLoadError::RequestIdExhausted),
                    session.options().clone()
                )
                .is_err()
        );
        assert!(session.artifacts().is_empty());
        assert_eq!(session.stylesheet().rule_count(), 0);
        let deps = session.dependencies();
        assert!(
            deps.candidates.is_empty()
                && deps.utilities.is_empty()
                && deps.variants.is_empty()
                && deps.properties.is_empty()
                && deps.theme_tokens.is_empty()
                && deps.assets.is_empty()
        );
        assert!(matches!(
            session.load_error(),
            Some(DesignSystemLoadError::RequestIdExhausted)
        ));
        assert!(
            session
                .reload(
                    Ok(reload_snapshot("@utility brand { color:blue; }")),
                    session.options().clone()
                )
                .unwrap()
                .diagnostics
                .is_empty()
        );
        assert!(session.load_error().is_none());
        assert!(session.artifacts().contains_key("motion.tsx"));
    }
}
