//! Mutable host orchestration; snapshots and in-flight request clones stay immutable.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AssetIdentity, CompileDiagnostic, CompiledModule, DesignSystemLoadError, DesignSystemSnapshot,
    GeneratedRule, ModuleAnalysis, ModuleCompiler, ModuleCssArtifact, ModuleId, RuleKey,
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SessionReloadError(#[from] pub DesignSystemLoadError);

#[derive(Clone, Debug)]
struct RuleEntry {
    rule: GeneratedRule,
    users: BTreeSet<ModuleId>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectStylesheet {
    rules: BTreeMap<RuleKey, RuleEntry>,
    contributions: BTreeMap<ModuleId, Vec<RuleKey>>,
}

impl ProjectStylesheet {
    pub fn replace(&mut self, module: ModuleId, artifact: &ModuleCssArtifact) {
        self.remove(&module);
        for rule in &artifact.rules {
            self.rules
                .entry(rule.key.clone())
                .or_insert_with(|| RuleEntry {
                    rule: rule.clone(),
                    users: BTreeSet::new(),
                })
                .users
                .insert(module.clone());
        }
        self.contributions.insert(
            module,
            artifact.rules.iter().map(|rule| rule.key.clone()).collect(),
        );
    }

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

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn module_css(&self) -> String {
        // Source-independent module ordering, preserving each emitter's canonical
        // candidate order and emitting shared rules only at their first owner.
        let mut emitted = BTreeSet::new();
        self.contributions
            .values()
            .flatten()
            .filter(|key| emitted.insert(*key))
            .map(|key| self.rules[key].rule.to_css())
            .collect()
    }

    pub fn serialize(&self, snapshot: &DesignSystemSnapshot) -> String {
        snapshot.render_stylesheet(&self.module_css())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMode {
    Build,
    Watch,
}

#[derive(Clone, Debug, Default)]
pub struct DependencyIndex {
    pub candidates: BTreeMap<String, BTreeSet<ModuleId>>,
    pub theme_tokens: BTreeMap<String, BTreeSet<ModuleId>>,
    pub utilities: BTreeMap<String, BTreeSet<ModuleId>>,
    pub variants: BTreeMap<String, BTreeSet<ModuleId>>,
    pub properties: BTreeMap<String, BTreeSet<ModuleId>>,
    pub assets: BTreeMap<AssetIdentity, BTreeSet<ModuleId>>,
}

#[derive(Clone, Debug, Default)]
pub struct SessionUpdate {
    pub css_changed: bool,
    pub transformed_modules: BTreeSet<ModuleId>,
    pub diagnostics: BTreeMap<ModuleId, Vec<CompileDiagnostic>>,
}

/// Exclusive mutation is the actor boundary. A host can put this in a mutex;
/// request handlers pin a clone obtained from `snapshot` before doing work.
pub struct ProjectSession {
    current: DesignSystemSnapshot,
    mode: SessionMode,
    analyses: BTreeMap<ModuleId, ModuleAnalysis>,
    artifacts: BTreeMap<ModuleId, CompiledModule>,
    stylesheet: ProjectStylesheet,
    dependencies: DependencyIndex,
    load_error: Option<DesignSystemLoadError>,
}

impl ProjectSession {
    pub fn new(snapshot: DesignSystemSnapshot, mode: SessionMode) -> Self {
        Self {
            current: snapshot,
            mode,
            analyses: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            stylesheet: ProjectStylesheet::default(),
            dependencies: DependencyIndex::default(),
            load_error: None,
        }
    }

    pub fn snapshot(&self) -> DesignSystemSnapshot {
        self.current.clone()
    }
    pub fn stylesheet(&self) -> &ProjectStylesheet {
        &self.stylesheet
    }
    pub fn artifacts(&self) -> &BTreeMap<ModuleId, CompiledModule> {
        &self.artifacts
    }
    pub fn analyses(&self) -> &BTreeMap<ModuleId, ModuleAnalysis> {
        &self.analyses
    }
    pub fn dependencies(&self) -> &DependencyIndex {
        &self.dependencies
    }
    pub fn load_error(&self) -> Option<&DesignSystemLoadError> {
        self.load_error.as_ref()
    }
    pub fn css(&self) -> String {
        self.stylesheet.serialize(&self.current)
    }

    pub fn update_module(&mut self, analysis: ModuleAnalysis) -> SessionUpdate {
        let backend = ModuleCompiler::new(self.current.clone());
        if analysis.diagnostics.is_empty()
            && analysis.integration_diagnostics.is_empty()
            && self
                .artifacts
                .get(&analysis.module_id)
                .is_some_and(|artifact| artifact.key == backend.key_for(&analysis))
        {
            self.analyses.insert(analysis.module_id.clone(), analysis);
            return SessionUpdate::default();
        }
        let old_css = self.css();
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
                compiled.diagnostics.push(CompileDiagnostic::new(
                    "SHSYMBOL001",
                    format!("generated identifier collision: {}", symbol.name),
                    analysis.source.full_span(),
                ));
            }
        }
        self.analyses.insert(module_id.clone(), analysis);
        let mut update = SessionUpdate::default();
        if compiled.is_success() {
            if self
                .artifacts
                .get(&module_id)
                .is_none_or(|previous| previous.code_transforms != compiled.code_transforms)
            {
                update.transformed_modules.insert(module_id.clone());
            }
            self.stylesheet.replace(module_id.clone(), &compiled.css);
            self.artifacts.insert(module_id, compiled);
        } else {
            update
                .diagnostics
                .insert(module_id.clone(), compiled.diagnostics);
            if self.mode == SessionMode::Build {
                self.stylesheet.remove(&module_id);
                self.artifacts.remove(&module_id);
            }
        }
        self.reindex();
        update.css_changed = old_css != self.css();
        update
    }

    pub fn remove_module(&mut self, module: &str) -> SessionUpdate {
        let old_css = self.css();
        self.analyses.remove(module);
        self.artifacts.remove(module);
        self.stylesheet.remove(module);
        self.reindex();
        SessionUpdate {
            css_changed: old_css != self.css(),
            ..SessionUpdate::default()
        }
    }

    /// Publishes an entirely rebuilt revision under the host's exclusive actor turn.
    ///
    /// # Errors
    /// Returns the attempted load failure. Watch mode retains all active output;
    /// build mode invalidates artifacts and requires the host to fail the build.
    pub fn reload(
        &mut self,
        result: Result<DesignSystemSnapshot, DesignSystemLoadError>,
    ) -> Result<SessionUpdate, SessionReloadError> {
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.load_error = Some(error.clone());
                if self.mode == SessionMode::Build {
                    self.artifacts.clear();
                    self.stylesheet = ProjectStylesheet::default();
                    self.reindex();
                }
                return Err(error.into());
            }
        };
        let old_css = self.css();
        let mut next = Self::new(snapshot, self.mode);
        let mut update = SessionUpdate::default();
        for (id, analysis) in &self.analyses {
            let changed = next.update_module(analysis.clone());
            update.diagnostics.extend(changed.diagnostics);
            if next.artifacts.get(id).map(|value| &value.code_transforms)
                != self.artifacts.get(id).map(|value| &value.code_transforms)
            {
                update.transformed_modules.insert(id.clone());
            }
        }
        if self.mode == SessionMode::Watch && !update.diagnostics.is_empty() {
            // A revision is published as one coherent set, never a mixture of
            // new registries and stale successful artifacts.
            update.transformed_modules.clear();
            return Ok(update);
        }
        update.css_changed = old_css != next.css();
        *self = next;
        Ok(update)
    }

    fn reindex(&mut self) {
        self.dependencies = DependencyIndex::default();
        for (id, artifact) in &self.artifacts {
            for (entries, index) in [
                (
                    &artifact.dependencies.theme_tokens,
                    &mut self.dependencies.theme_tokens,
                ),
                (
                    &artifact.dependencies.utilities,
                    &mut self.dependencies.utilities,
                ),
                (
                    &artifact.dependencies.variants,
                    &mut self.dependencies.variants,
                ),
                (
                    &artifact.dependencies.properties,
                    &mut self.dependencies.properties,
                ),
            ] {
                for entry in entries {
                    index.entry(entry.clone()).or_default().insert(id.clone());
                }
            }
            for asset in &artifact.dependencies.assets {
                self.dependencies
                    .assets
                    .entry(asset.clone())
                    .or_default()
                    .insert(id.clone());
            }
            for candidate in &artifact.dependencies.candidates {
                self.dependencies
                    .candidates
                    .entry(candidate.clone())
                    .or_default()
                    .insert(id.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::{FileId, SourceFile};

    use crate::{CompilerOptions, OxcSourceAnalyzer, SourceAnalyzer};

    use super::*;

    #[test]
    fn removing_one_module_does_not_drop_rules_shared_by_another() {
        let artifact = ModuleCssArtifact::from_css(".x{color:red;}");
        let mut stylesheet = ProjectStylesheet::default();
        stylesheet.replace("a".into(), &artifact);
        stylesheet.replace("b".into(), &artifact);
        assert_eq!(stylesheet.rule_count(), 1);
        stylesheet.remove("a");
        assert_eq!(stylesheet.module_css(), ".x{color:red;}");
        stylesheet.remove("b");
        assert_eq!(stylesheet.rule_count(), 0);
        stylesheet.remove("absent");
        assert_eq!(stylesheet.module_css(), "");
    }

    fn reload_snapshot(css: &str) -> DesignSystemSnapshot {
        DesignSystemSnapshot::from_source(
            &SourceFile::new(FileId(2), "theme", css),
            CompilerOptions::default(),
        )
        .unwrap()
    }

    fn reload_module() -> ModuleAnalysis {
        let source = SourceFile::new(
            FileId(1),
            "motion.tsx",
            "import { style } from 'stillwind/react'; const motion = style.animation({ from: 'brand' }); const x = style(motion('both')); ",
        );
        OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(source)), None)
    }

    #[test]
    fn watch_reload_rejects_the_entire_revision_when_recompilation_fails() {
        let mut session = ProjectSession::new(
            reload_snapshot("@utility brand { color:red; }"),
            SessionMode::Watch,
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
        );
        let analysis =
            OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(other)), None);
        assert!(session.update_module(analysis).diagnostics.is_empty());
        let other_artifact = session.artifacts()["other.tsx"].css.to_css();
        let before_css = session.css();
        let revision = session.snapshot().revision();
        let before_artifact = session.artifacts()["motion.tsx"].css.to_css();
        let update = session.reload(Ok(reload_snapshot(""))).unwrap();
        assert!(
            update.diagnostics["motion.tsx"]
                .iter()
                .any(|error| error.code == "SH001")
        );
        assert!(!update.css_changed);
        assert!(update.transformed_modules.is_empty());
        assert_eq!(session.snapshot().revision(), revision);
        assert_eq!(session.css(), before_css);
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
                .reload(Ok(reload_snapshot("@utility brand { color:blue; }")))
                .unwrap()
                .css_changed
        );
    }

    #[test]
    fn build_load_failure_clears_artifacts_and_dependency_indexes() {
        let mut session = ProjectSession::new(
            reload_snapshot("@utility brand { color:red; }"),
            SessionMode::Build,
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
                .reload(Err(DesignSystemLoadError::RequestIdExhausted))
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
                .reload(Ok(reload_snapshot("@utility brand { color:blue; }")))
                .unwrap()
                .diagnostics
                .is_empty()
        );
        assert!(session.load_error().is_none());
        assert!(session.artifacts().contains_key("motion.tsx"));
    }
}
