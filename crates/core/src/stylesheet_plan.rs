//! Explicit host selections; observing transforms never proves graph completeness.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Fingerprint, ModuleArtifactKey, ModuleId, ProjectSession, ProjectStylesheet, StylesheetError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StylesheetCoverage {
    Observed,
    /// The host guarantees this selection for this target and graph revision.
    Complete {
        target: String,
        revision: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StylesheetPlacement {
    /// Includes authored document CSS as well as selected generated rules.
    Document,
    /// Includes generated rules and their design-system support, without authored globals.
    Carrier,
}

#[derive(Clone, Debug)]
pub struct StylesheetSelection {
    pub modules: BTreeSet<ModuleId>,
    pub coverage: StylesheetCoverage,
}

#[derive(Clone, Debug)]
pub struct StylesheetPlan {
    pub css: String,
    pub coverage: StylesheetCoverage,
    pub placement: StylesheetPlacement,
    pub design_system: Fingerprint,
    pub producers: BTreeMap<ModuleId, ModuleArtifactKey>,
}

#[derive(Debug, thiserror::Error)]
pub enum StylesheetPlanError {
    #[error("Stillwind selection contains unregistered modules: {0:?}")]
    MissingModules(BTreeSet<ModuleId>),
    #[error("Complete coverage requires a nonempty host target and graph revision")]
    MissingCoverageIdentity,
    #[error("Stillwind selection contains stale producers: {0:?}")]
    StaleModules(BTreeSet<ModuleId>),
    #[error("The latest design-system load failed; retained CSS is not a current output plan")]
    FailedDesignSystemLoad,
    #[error("{0}")]
    Stylesheet(#[from] StylesheetError),
}

impl ProjectSession {
    /// A snapshot of observed producers, explicitly carrying no completeness guarantee.
    pub fn observed_selection(&self) -> StylesheetSelection {
        StylesheetSelection {
            modules: self.artifacts().keys().cloned().collect(),
            coverage: StylesheetCoverage::Observed,
        }
    }

    /// Plans a module, chunk, entry, or document selection supplied by the host.
    ///
    /// # Errors
    /// Rejects unavailable producers, incomplete coverage metadata, or invalid CSS.
    pub fn plan_stylesheet(
        &self,
        selection: StylesheetSelection,
        placement: StylesheetPlacement,
    ) -> Result<StylesheetPlan, StylesheetPlanError> {
        if self.load_error().is_some() {
            return Err(StylesheetPlanError::FailedDesignSystemLoad);
        }
        if let StylesheetCoverage::Complete { target, revision } = &selection.coverage
            && (target.trim().is_empty() || revision.trim().is_empty())
        {
            return Err(StylesheetPlanError::MissingCoverageIdentity);
        }
        let missing = selection
            .modules
            .iter()
            .filter(|id| !self.artifacts().contains_key(*id))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing.is_empty() {
            return Err(StylesheetPlanError::MissingModules(missing));
        }
        let compiler = crate::ModuleCompiler::new(self.snapshot());
        let stale = selection
            .modules
            .iter()
            .filter(
                |id| match (self.analyses().get(*id), self.artifacts().get(*id)) {
                    (Some(analysis), Some(artifact)) => {
                        !analysis.diagnostics.is_empty()
                            || !analysis.integration_diagnostics.is_empty()
                            || artifact.key != compiler.key_for(analysis)
                    }
                    _ => true,
                },
            )
            .cloned()
            .collect::<BTreeSet<_>>();
        if !stale.is_empty() {
            return Err(StylesheetPlanError::StaleModules(stale));
        }
        let mut stylesheet = ProjectStylesheet::default();
        let mut producers = BTreeMap::new();
        for module in &selection.modules {
            if let Some(artifact) = self.artifacts().get(module) {
                stylesheet.replace(module.clone(), &artifact.css);
                producers.insert(module.clone(), artifact.key.clone());
            }
        }
        let snapshot = self.snapshot();
        let css = match placement {
            StylesheetPlacement::Document => stylesheet.serialize(&snapshot)?,
            StylesheetPlacement::Carrier => {
                snapshot.render_module_stylesheet(&stylesheet.module_css()?)?
            }
        };
        Ok(StylesheetPlan {
            css,
            coverage: selection.coverage,
            placement,
            design_system: snapshot.fingerprints().content.clone(),
            producers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompilerOptions, DesignSystemSnapshot, OxcSourceAnalyzer, SessionMode, SourceAnalyzer,
    };
    use std::sync::Arc;

    #[test]
    fn retained_watch_output_cannot_be_reported_as_current_coverage() {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(
                FileId(0),
                "design.css",
                "@utility red{color:red}@stillwind-slot utilities;",
            )
            .unwrap(),
            CompilerOptions::default(),
        )
        .unwrap();
        let mut session = ProjectSession::new(snapshot, SessionMode::Watch);
        for utility in ["'red'", "unknown"] {
            let source = SourceFile::new(
                FileId(1),
                "entry.ts",
                format!("import {{style}} from 'stillwind/react'; const x=style({utility});"),
            )
            .unwrap();
            let analysis =
                OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(Arc::new(source)), None);
            session.update_module(analysis).unwrap();
        }
        assert!(session.css().unwrap().contains("color:red"));
        assert!(matches!(
            session.plan_stylesheet(session.observed_selection(), StylesheetPlacement::Document),
            Err(StylesheetPlanError::StaleModules(_))
        ));
    }
    use stillwind_source::{FileId, SourceFile};

    #[test]
    fn explicit_selection_excludes_observed_but_unselected_producers() {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(FileId(0), "design.css", "body{margin:0}@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;").unwrap(),
            CompilerOptions::default(),
        ).unwrap();
        let mut session = ProjectSession::new(snapshot, SessionMode::Build);
        for (index, name) in ["red", "blue"].iter().enumerate() {
            let source = SourceFile::new(
                FileId(u32::try_from(index).unwrap() + 1),
                format!("{name}.tsx"),
                format!("import {{style}} from 'stillwind/react'; export const x=style('{name}');"),
            )
            .unwrap();
            let analysis =
                OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(Arc::new(source)), None);
            assert!(
                session
                    .update_module(analysis)
                    .unwrap()
                    .diagnostics
                    .is_empty()
            );
        }
        let observed = session
            .plan_stylesheet(session.observed_selection(), StylesheetPlacement::Document)
            .unwrap();
        assert_eq!(observed.coverage, StylesheetCoverage::Observed);
        assert!(observed.css.contains("color:red") && observed.css.contains("color:blue"));
        let selection = StylesheetSelection {
            modules: ["red.tsx".into()].into(),
            coverage: StylesheetCoverage::Complete {
                target: "route/index".into(),
                revision: "graph-2".into(),
            },
        };
        let carrier = session
            .plan_stylesheet(selection.clone(), StylesheetPlacement::Carrier)
            .unwrap();
        assert!(carrier.css.contains("color:red"));
        assert!(!carrier.css.contains("color:blue") && !carrier.css.contains("body"));
        assert_eq!(carrier.producers.len(), 1);
        let document = session
            .plan_stylesheet(selection.clone(), StylesheetPlacement::Document)
            .unwrap();
        assert!(document.css.contains("body"));
        session.remove_module("red.tsx").unwrap();
        assert!(matches!(
            session.plan_stylesheet(selection, StylesheetPlacement::Carrier),
            Err(StylesheetPlanError::MissingModules(_))
        ));
        assert_eq!(session.observed_selection().modules.len(), 1);
        assert!(matches!(
            session.plan_stylesheet(
                StylesheetSelection {
                    modules: BTreeSet::new(),
                    coverage: StylesheetCoverage::Complete {
                        target: String::new(),
                        revision: String::new()
                    }
                },
                StylesheetPlacement::Document
            ),
            Err(StylesheetPlanError::MissingCoverageIdentity)
        ));
    }
}
