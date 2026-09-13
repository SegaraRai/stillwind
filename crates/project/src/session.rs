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
use std::sync::Arc;

use stillwind_compiler::{
    CompiledModule, CompilerOptions, GeneratedRule, MappedStylesheet, ModuleCompiler,
    ModuleCssArtifact, RuleKey, StylesheetError, StylesheetRenderer,
};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemSnapshot};
use stillwind_ir::ModuleAnalysis;
use stillwind_semantic::ModuleId;
use stillwind_source::{CompileError, SourceFile, Span};

use crate::DependencyIndex;

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
        StylesheetError,
    ),
}

/// One generated rule plus the set of producers that depend on it.
///
/// The rule body is shared; only the membership varies per module. Removing a producer
/// therefore never removes a rule another producer still needs.
#[derive(Clone, Debug)]
struct RuleEntry {
    /// Shared generated rule retained while at least one module uses it.
    rule: GeneratedRule,
    /// Module identities currently contributing this rule; set order is module-id order.
    users: BTreeSet<ModuleId>,
}

/// The project-wide stylesheet assembled from published module artifacts.
///
/// Module updates retract their previous ownership without removing rules still used
/// by other modules.
#[derive(Clone, Debug, Default)]
pub struct ProjectStylesheet {
    /// Content-identity lookup of rules and their current producers.
    rules: BTreeMap<RuleKey, RuleEntry>,
    /// Rule keys per module, in that artifact's emission order, used to retract its
    /// previous contribution.
    contributions: BTreeMap<ModuleId, Vec<RuleKey>>,
}

impl ProjectStylesheet {
    /// Replaces one module's contribution with the rules of `artifact`.
    ///
    /// Rules already contributed by another module are shared, not duplicated; keys the
    /// module no longer claims are dropped when their last user goes.
    /// Publicly constructed or edited rules are re-keyed from their current content.
    pub fn replace(&mut self, module: ModuleId, artifact: &ModuleCssArtifact) {
        self.remove(module.as_str());
        let mut keys = Vec::with_capacity(artifact.rules.len());
        for rule in &artifact.rules {
            let key = rule.content_key();
            keys.push(key.clone());
            self.rules
                .entry(key.clone())
                .or_insert_with(|| {
                    let mut rule = rule.clone();
                    rule.key = key;
                    RuleEntry {
                        rule,
                        users: BTreeSet::new(),
                    }
                })
                .users
                .insert(module.clone());
        }
        self.contributions.insert(module, keys);
    }

    /// Withdraws every rule this module contributed, keeping rules that other modules
    /// still use. Unknown modules are ignored.
    pub fn remove(&mut self, module: &str) {
        if let Some(keys) = self.contributions.remove(module) {
            for key in keys {
                if let Some(entry) = self.rules.get_mut(&key) {
                    entry.users.remove(module);
                    if entry.users.is_empty() {
                        self.rules.remove(&key);
                    }
                }
            }
        }
    }

    /// Number of distinct rules currently retained.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Serializes the retained module CSS without design-system wrapper rules.
    ///
    /// This is the input to [`ProjectStylesheet::serialize`]; hosts that need the
    /// module fragment alone, such as a CSS carrier module, use it directly.
    ///
    /// # Errors
    ///
    /// Returns an error if a contribution refers to a missing rule.
    pub fn module_css(&self) -> Result<String, StylesheetError> {
        let mapped = self.module_css_with_origins(|_, _| None)?;
        Ok(mapped
            .keyframes
            .iter()
            .map(|frames| frames.css.as_str())
            .chain(std::iter::once(mapped.css.as_str()))
            .collect())
    }

    /// Like [`ProjectStylesheet::module_css`] but reports a source location for each
    /// rule so the renderer can emit accurate source maps.
    ///
    /// The callback receives the producing module and the rule and returns the original
    /// file and span to attribute it to. Returning `None` leaves the rule unmapped; a
    /// rule is never attributed to a location that belongs to a different file.
    pub(crate) fn module_css_with_origins(
        &self,
        mut origin: impl FnMut(&ModuleId, &GeneratedRule) -> Option<(Arc<SourceFile>, Span)>,
    ) -> Result<MappedStylesheet, StylesheetError> {
        let mut emitted = BTreeSet::new();
        let mut css = MappedStylesheet::default();
        for (module, keys) in &self.contributions {
            for key in keys {
                if emitted.insert(key) {
                    let entry = self.rules.get(key).ok_or(StylesheetError::MissingRule)?;
                    entry.rule.append_to(&mut css, origin(module, &entry.rule));
                }
            }
        }
        Ok(css)
    }

    /// Renders the full project stylesheet: module CSS plus the design system's own
    /// rules, wrappers, and authored directives.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent contributions or invalid assembled CSS.
    pub fn serialize(
        &self,
        snapshot: &DesignSystemSnapshot,
        options: &CompilerOptions,
    ) -> Result<String, StylesheetError> {
        StylesheetRenderer::new(snapshot, options)
            .render_mapped_stylesheet(
                &self.module_css_with_origins(|_, _| None)?,
                stillwind_compiler::StylesheetPlacement::Document,
            )
            .map(|css| css.css)
    }
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
    pub fn css(&self) -> Result<String, StylesheetError> {
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
                    "SHSYMBOL001",
                    format!("generated identifier collision: {}", symbol.name),
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
    use stillwind_source::{FileId, Fingerprint, SourceFile};

    use super::*;

    #[test]
    fn support_metadata_participates_in_ownership_and_survives_removal() {
        let plain = GeneratedRule {
            key: RuleKey(Fingerprint::of("stale")),
            prelude: ".x".into(),
            body: Some("transform:var(--x)".into()),
            keyframes: false,
            required_properties: Vec::new(),
        };
        let mut supported = plain.clone();
        supported.required_properties.push("--x".into());
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet.replace("plain".into(), &ModuleCssArtifact { rules: vec![plain] });
        stylesheet.replace(
            "supported".into(),
            &ModuleCssArtifact {
                rules: vec![supported],
            },
        );
        assert_eq!(stylesheet.rules.len(), 2);
        assert_eq!(
            stylesheet
                .module_css_with_origins(|_, _| None)
                .unwrap()
                .required_properties,
            ["--x"]
        );
        stylesheet.remove("supported");
        assert_eq!(stylesheet.rules.len(), 1);
        assert!(
            stylesheet
                .module_css_with_origins(|_, _| None)
                .unwrap()
                .required_properties
                .is_empty()
        );
    }

    #[test]
    fn missing_rule_is_a_serialization_error() {
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet
            .contributions
            .insert("x".into(), vec![RuleKey(Fingerprint::of("missing"))]);
        assert!(matches!(
            stylesheet.module_css(),
            Err(StylesheetError::MissingRule)
        ));
    }

    #[test]
    fn removing_one_module_does_not_drop_rules_shared_by_another() {
        let artifact = ModuleCssArtifact {
            rules: vec![GeneratedRule {
                keyframes: false,
                required_properties: Vec::new(),
                key: RuleKey(Fingerprint::of(".x{color:red;}")),
                prelude: ".x".into(),
                body: Some("color:red;".into()),
            }],
        };
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet.replace("a".into(), &artifact);
        stylesheet.replace("b".into(), &artifact);
        assert_eq!(stylesheet.rule_count(), 1);
        stylesheet.remove("a");
        assert_eq!(stylesheet.module_css().unwrap(), ".x{color:red;}");
        stylesheet.remove("b");
        assert_eq!(stylesheet.rule_count(), 0);
        stylesheet.remove("absent");
        assert_eq!(stylesheet.module_css().unwrap(), "");
    }

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
                .any(|error| error.code == "SH001")
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
