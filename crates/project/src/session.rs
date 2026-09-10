//! Mutable host orchestration; snapshots and in-flight request clones stay immutable.

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

#[derive(Debug, thiserror::Error)]
pub enum SessionReloadError {
    #[error("{0}")]
    Load(#[from] DesignSystemLoadError),
    #[error("{0}")]
    Stylesheet(#[from] StylesheetError),
}

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
        self.remove(module.as_str());
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

    /// # Errors
    /// Returns an error if a contribution refers to a missing rule.
    pub fn module_css(&self) -> Result<String, StylesheetError> {
        Ok(self.module_css_with_origins(|_, _| None)?.css)
    }

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
                    css.text(&entry.rule.to_css(), origin(module, &entry.rule));
                }
            }
        }
        Ok(css)
    }

    /// # Errors
    /// Returns an error for inconsistent contributions or invalid assembled CSS.
    pub fn serialize(
        &self,
        snapshot: &DesignSystemSnapshot,
        options: &CompilerOptions,
    ) -> Result<String, StylesheetError> {
        StylesheetRenderer::new(snapshot, options).render_stylesheet(&self.module_css()?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMode {
    Build,
    Watch,
}

#[derive(Clone, Debug, Default)]
pub struct ModuleUpdate {
    /// Whether a successful update published different source transforms.
    pub transformed: bool,
    pub diagnostics: Vec<CompileError>,
}

#[derive(Clone, Debug, Default)]
pub struct SessionReload {
    pub css_changed: bool,
    pub transformed_modules: BTreeSet<ModuleId>,
    pub diagnostics: BTreeMap<ModuleId, Vec<CompileError>>,
}

/// Exclusive mutation is the actor boundary. A host can put this in a mutex;
/// request handlers pin a clone obtained from `snapshot` before doing work.
pub struct ProjectSession {
    current: DesignSystemSnapshot,
    options: CompilerOptions,
    mode: SessionMode,
    analyses: BTreeMap<ModuleId, ModuleAnalysis>,
    artifacts: BTreeMap<ModuleId, CompiledModule>,
    stylesheet: ProjectStylesheet,
    dependencies: DependencyIndex,
    load_error: Option<DesignSystemLoadError>,
}

impl ProjectSession {
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

    pub fn options(&self) -> &CompilerOptions {
        &self.options
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
    /// # Errors
    /// Returns an error if stylesheet serialization fails.
    pub fn css(&self) -> Result<String, StylesheetError> {
        self.stylesheet.serialize(&self.current, &self.options)
    }

    /// Updates one producer without rendering the project stylesheet.
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

    /// Removes a producer, returning whether it was registered.
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
    /// # Errors
    /// Returns the attempted load failure. Watch mode retains all active output;
    /// build mode invalidates artifacts and requires the host to fail the build.
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
            // A revision is published as one coherent set, never a mixture of
            // new registries and stale successful artifacts.
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
