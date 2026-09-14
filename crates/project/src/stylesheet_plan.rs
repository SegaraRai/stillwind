//! Explicit host selections; observing transforms never proves graph completeness.
//!
//! A selection names the producers whose CSS belongs in one output target, and a plan
//! is the rendered result plus the provenance a host needs to attribute and watch it.
//! The completeness question is deliberately not answered here: the compiler can see
//! which modules it transformed, but only the host knows whether that set is the whole
//! module graph for a target. A selection therefore states its coverage explicitly, and
//! a plan for retained output after a failed load is rejected rather than reported as
//! current.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{ParseError, Parser, Token};

use stillwind_compiler::{
    ModuleArtifactKey, ModuleCompiler, ModuleResourceReference, StylesheetMapping,
    StylesheetPlacement, StylesheetRenderer,
};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_semantic::ModuleId;
use stillwind_source::{Fingerprint, Span};

use crate::{ProjectSession, ProjectStylesheet};

/// Why a stylesheet plan could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum StylesheetPlanError {
    /// The selection names modules with no published artifact.
    #[error("Stillwind selection contains unregistered modules: {0:?}")]
    MissingModules(BTreeSet<ModuleId>),
    /// A complete-coverage claim came without a target or graph revision.
    #[error("Complete coverage requires a nonempty host target and graph revision")]
    MissingCoverageIdentity,
    /// A selected artifact was compiled against a different snapshot or failed its
    /// current analysis, so its CSS is not a valid output for this plan.
    #[error("Stillwind selection contains stale producers: {0:?}")]
    StaleModules(BTreeSet<ModuleId>),
    /// The retained revision follows a failed load, so it is not a current plan.
    #[error("The latest design-system load failed; retained CSS is not a current output plan")]
    FailedDesignSystemLoad,
    /// Rendering the selected CSS failed.
    #[error("{0}")]
    Stylesheet(#[from] CompileErrorKind),
}

/// How much the host knows about the selected producer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StylesheetCoverage {
    /// The selection is what this session has seen so far. It makes no claim about
    /// modules that were never compiled, so it cannot justify emitting a complete
    /// target stylesheet.
    Observed,
    /// The host guarantees this selection for this target and graph revision.
    ///
    /// Both fields must be non-empty: they are the host's statement of which build
    /// output and which module-graph revision the claim belongs to, and they travel
    /// into the plan so a consumer can detect a stale claim.
    Complete {
        /// Host-defined identity of the output being planned, for example a route id.
        target: String,
        /// Host-defined revision of the module graph the selection came from.
        revision: String,
    },
}

/// The producers one output should contain, with their coverage claim.
#[derive(Clone, Debug)]
pub struct StylesheetSelection {
    /// Modules to include. Order does not matter; rules keep their own order.
    pub modules: BTreeSet<ModuleId>,
    /// What the host knows about the completeness of `modules`.
    pub coverage: StylesheetCoverage,
}

/// A rendered stylesheet plus everything needed to place and attribute it.
#[derive(Clone, Debug)]
pub struct StylesheetPlan {
    /// Generated-to-original mappings for the rules that have a source location.
    pub mappings: Vec<StylesheetMapping>,
    /// Assets and resources the selected producers reference.
    pub resources: Vec<ModuleResourceReference>,
    /// The rendered CSS text.
    pub css: String,
    /// Coverage claim carried over from the selection.
    pub coverage: StylesheetCoverage,
    /// Where the host should place this stylesheet.
    pub placement: StylesheetPlacement,
    /// Content fingerprint of the design system this CSS was rendered against.
    pub design_system: Fingerprint,
    /// Artifact key per selected module, for cache identity and invalidation.
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
    /// Rejects the request instead of guessing when a selected module has no artifact,
    /// when its artifact is stale with respect to the current snapshot, or when
    /// coverage is claimed without a target and revision. Staleness is checked per
    /// module so the error names exactly the producers the host must recompile.
    ///
    /// # Errors
    ///
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

/// Collects identifier tokens from generated CSS.
///
/// Matching is done on parsed CSS tokens rather than raw text so that a class name
/// appearing inside a string, a comment, or an escape sequence cannot be mistaken for
/// the generated identifier; locations themselves come from compiler metadata, never
/// from this scan.
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
