//! Explicit host selections; observing transforms never proves graph completeness.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{ParseError, Parser, Token};

use stillwind_compiler::{
    ModuleArtifactKey, ModuleCompiler, ModuleResourceReference, StylesheetError, StylesheetMapping,
    StylesheetPlacement, StylesheetRenderer,
};
use stillwind_semantic::ModuleId;
use stillwind_source::{Fingerprint, Span};

use crate::{ProjectSession, ProjectStylesheet};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StylesheetCoverage {
    Observed,
    /// The host guarantees this selection for this target and graph revision.
    Complete {
        target: String,
        revision: String,
    },
}

#[derive(Clone, Debug)]
pub struct StylesheetSelection {
    pub modules: BTreeSet<ModuleId>,
    pub coverage: StylesheetCoverage,
}

#[derive(Clone, Debug)]
pub struct StylesheetPlan {
    pub mappings: Vec<StylesheetMapping>,
    pub resources: Vec<ModuleResourceReference>,
    pub css: String,
    pub coverage: StylesheetCoverage,
    pub placement: StylesheetPlacement,
    pub design_system: Fingerprint,
    pub producers: BTreeMap<ModuleId, ModuleArtifactKey>,
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
        let compiler = ModuleCompiler::new(self.snapshot(), self.options().clone());
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
        let mut resources = Vec::new();
        for module in &selection.modules {
            if let Some(artifact) = self.artifacts().get(module) {
                stylesheet.replace(module.clone(), &artifact.css);
                producers.insert(module.clone(), artifact.key.clone());
                resources.extend(artifact.resources.iter().cloned());
            }
        }
        let snapshot = self.snapshot();
        let utility_css = stylesheet.module_css_with_origins(|module, rule| {
            let artifact = self.artifacts().get(module)?;
            let analysis = self.analyses().get(module)?;
            let identifiers = rule_identifiers(&rule.to_css());
            if let Some(class) = artifact
                .classes
                .iter()
                .find(|class| identifiers.contains(&class.class_name))
            {
                return Some((analysis.source.clone(), class.call_span));
            }
            let symbol = artifact
                .symbols
                .iter()
                .find(|symbol| identifiers.contains(&symbol.name))?;
            let source = std::iter::once(analysis)
                .chain(analysis.asset_support.iter())
                .find(|input| input.source.path() == symbol.source_path)?
                .source
                .clone();
            let span = Span::new(source.id(), symbol.start, symbol.end);
            Some((source, span))
        })?;
        let mapped = StylesheetRenderer::new(&snapshot, self.options())
            .render_mapped_stylesheet(&utility_css, placement)?;
        Ok(StylesheetPlan {
            mappings: mapped.mappings,
            resources,
            css: mapped.css,
            coverage: selection.coverage,
            placement,
            design_system: snapshot.fingerprints().content.clone(),
            producers,
        })
    }
}

// Use CSS tokens so strings, comments and identifier escapes cannot masquerade
// as a generated identifier. Locations themselves come from compiler metadata.
fn rule_identifiers(css: &str) -> BTreeSet<String> {
    fn visit(parser: &mut Parser<'_>, names: &mut BTreeSet<String>) {
        while let Ok(token) = parser.next() {
            match token {
                Token::Ident(name) => {
                    names.insert(name.to_string());
                }
                Token::CurlyBracketBlock
                | Token::SquareBracketBlock
                | Token::ParenthesisBlock
                | Token::Function(_) => {
                    let _ = parser.parse_nested_block(|nested| {
                        visit(nested, names);
                        Ok::<_, ParseError<()>>(())
                    });
                }
                _ => {}
            }
        }
    }
    let mut names = BTreeSet::new();
    visit(&mut Parser::new(css), &mut names);
    names
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_compiler::CompilerOptions;
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_ir::SourceAnalyzer;
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{FileId, SourceFile};

    use crate::SessionMode;

    use super::*;

    #[test]
    fn retained_watch_output_cannot_be_reported_as_current_coverage() {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(
                FileId(0),
                "design.css",
                "@utility red{color:red}@stillwind-slot utilities;",
            )
            .unwrap(),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let mut session =
            ProjectSession::new(snapshot, SessionMode::Watch, CompilerOptions::default());
        for utility in ["'red'", "unknown"] {
            let source = SourceFile::new(
                FileId(1),
                "entry.ts",
                format!("import {{style}} from 'stillwind/react'; const x=style({utility});"),
            )
            .unwrap();
            let analysis =
                OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(Arc::new(source)), None);
            session.update_module(analysis);
        }
        assert!(session.css().unwrap().contains("color:red"));
        assert!(matches!(
            session.plan_stylesheet(session.observed_selection(), StylesheetPlacement::Document),
            Err(StylesheetPlanError::StaleModules(_))
        ));
    }

    #[test]
    fn explicit_selection_excludes_observed_but_unselected_producers() {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(FileId(0), "design.css", "body{margin:0}@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;").unwrap(),
            DesignSystemOptions::default(),
        ).unwrap();
        let mut session =
            ProjectSession::new(snapshot, SessionMode::Build, CompilerOptions::default());
        for (index, name) in ["red", "blue"].iter().enumerate() {
            let source = SourceFile::new(
                FileId(u32::try_from(index).unwrap() + 1),
                format!("{name}.tsx"),
                format!("import {{style}} from 'stillwind/react'; export const x=style('{name}');"),
            )
            .unwrap();
            let analysis =
                OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(Arc::new(source)), None);
            assert!(session.update_module(analysis).diagnostics.is_empty());
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
        session.remove_module("red.tsx");
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
