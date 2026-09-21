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
    MappedStylesheet, ModuleArtifactKey, ModuleCompiler, ModuleResourceReference,
    StylesheetMapping, StylesheetPlacement, StylesheetRenderer,
};
use stillwind_css::{CssRule, optimize_rules, stylesheet_fragments};
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
    /// Producer dependencies contain a cycle.
    #[error("Cyclic stylesheet producer dependencies: {0:?}")]
    CyclicModules(BTreeSet<ModuleId>),
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
    /// Fragments in reference order. Repeated content remains a separate entry.
    pub parts: Vec<StylesheetPart>,
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

/// A shareable payload at one position in a stylesheet plan.
#[derive(Clone, Debug)]
pub struct StylesheetPart {
    /// Source attribution relative to the complete wrapped fragment.
    pub mappings: Vec<StylesheetMapping>,
    /// Stable template node ordinal.
    pub slot: usize,
    /// Zero-based occurrence index for this template slot and producer set.
    pub sequence: usize,
    /// Producers whose source ranges contribute to this fragment. Empty means shared support.
    pub producers: BTreeSet<ModuleId>,
    /// Content identity, suitable for an immutable fragment cache.
    pub key: Fingerprint,
    /// CSS for this occurrence, including its complete enclosing context.
    pub css: String,
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
        mut selection: StylesheetSelection,
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
        let order = self.stylesheet_order(&mut selection.modules)?;
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
        stylesheet.set_order(order);
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
            // A name without a declaration, such as a conditional helper, has no position in the
            // module to map the rule back to.
            let (start, end) = symbol.start.zip(symbol.end)?;
            let source = std::iter::once(analysis)
                .chain(analysis.asset_support.iter())
                .find(|input| input.source.path() == symbol.source_path)?
                .source
                .clone();
            let span = Span::new(source.id(), start, end);
            Some((source, span))
        });
        let renderer = StylesheetRenderer::new(&snapshot, self.options(), self.identifiers());
        let reference = renderer.render_mapped_stylesheet(&utility_css, placement)?;
        let mapped = if matches!(selection.coverage, StylesheetCoverage::Complete { .. }) {
            let optimized = optimize_stylesheet(&utility_css)?;
            renderer.render_mapped_stylesheet(&optimized, placement)?
        } else {
            reference.clone()
        };
        let parts = stylesheet_parts(&reference, &producers)?;
        // The selected producers were checked when they were published; the names this render
        // generated belong to no declaration, so they are checked against the document here.
        self.check_document_names(&renderer.take_detached_symbols())?;
        Ok(StylesheetPlan {
            parts,
            mappings: mapped.mappings,
            resources,
            css: mapped.css,
            coverage: selection.coverage,
            placement,
            design_system: snapshot.fingerprints().content.clone(),
            producers,
        })
    }

    /// Adds transitive base producers from composition edges to `modules` and returns
    /// their dependency order, breaking ties between ready producers by module identity.
    ///
    /// # Errors
    ///
    /// Rejects a cyclic producer graph. Missing producers are validated by planning.
    pub fn stylesheet_order(
        &self,
        modules: &mut BTreeSet<ModuleId>,
    ) -> Result<Vec<ModuleId>, StylesheetPlanError> {
        let mut pending: Vec<_> = modules.iter().cloned().collect();
        while let Some(module) = pending.pop() {
            if let Some(artifact) = self.artifacts().get(&module) {
                for dependency in &artifact.composition {
                    let base = ModuleId::from(dependency.base.module_path.clone());
                    if modules.insert(base.clone()) {
                        pending.push(base);
                    }
                }
            }
        }
        let mut remaining = modules.clone();
        let mut order = Vec::new();
        while !remaining.is_empty() {
            let ready = remaining
                .iter()
                .find(|module| {
                    self.artifacts().get(*module).is_none_or(|artifact| {
                        artifact.composition.iter().all(|edge| {
                            edge.base.module_path == module.as_str()
                                || !remaining.contains(edge.base.module_path.as_str())
                        })
                    })
                })
                .cloned();
            let Some(module) = ready else {
                return Err(StylesheetPlanError::CyclicModules(remaining));
            };
            remaining.remove(&module);
            order.push(module);
        }
        Ok(order)
    }
}

fn optimize_stylesheet(
    utility_css: &MappedStylesheet,
) -> Result<MappedStylesheet, StylesheetPlanError> {
    let mut optimized = utility_css.clone();
    let mut syntax = CssRule::parse(&utility_css.css).map_err(|offset| {
        StylesheetPlanError::Stylesheet(CompileErrorKind::InvalidGeneratedCss { offset })
    })?;
    optimize_rules(&mut syntax);
    optimized.css.clear();
    optimized.mappings.clear();
    for node in syntax {
        let start = optimized.css.len();
        optimized.css.push_str(&node.to_css());
        if let Some(mapping) = utility_css.mappings.iter().find(|mapping| {
            mapping.generated.start < node.range.end && mapping.generated.end > node.range.start
        }) {
            let mut mapping = mapping.clone();
            mapping.generated = start..optimized.css.len();
            mapping.exact = false;
            optimized.mappings.push(mapping);
        }
    }
    Ok(optimized)
}

fn stylesheet_parts(
    reference: &MappedStylesheet,
    producers: &BTreeMap<ModuleId, ModuleArtifactKey>,
) -> Result<Vec<StylesheetPart>, StylesheetPlanError> {
    let mut sequences = BTreeMap::new();
    let parts = stylesheet_fragments(&reference.css)
        .map_err(|offset| {
            StylesheetPlanError::Stylesheet(CompileErrorKind::InvalidGeneratedCss { offset })
        })?
        .into_iter()
        .map(|(css, range)| {
            let owners: BTreeSet<_> = reference
                .mappings
                .iter()
                .filter(|mapping| {
                    mapping.generated.start < range.end && mapping.generated.end > range.start
                })
                .map(|mapping| ModuleId::from(mapping.source.path()))
                .filter(|id| producers.contains_key(id))
                .collect();
            let slot = reference
                .segments
                .iter()
                .filter(|(segment, _)| segment.start < range.end && segment.end > range.start)
                .map(|(_, slot)| *slot)
                .min()
                .unwrap_or_default();
            let sequence = sequences.entry((slot, owners.clone())).or_insert(0);
            let prefix = css.rfind(&reference.css[range.clone()]).unwrap_or_default();
            let mappings = reference
                .mappings
                .iter()
                .filter_map(|mapping| {
                    let start = mapping.generated.start.max(range.start);
                    let end = mapping.generated.end.min(range.end);
                    if start >= end {
                        return None;
                    }
                    let mut mapped = mapping.clone();
                    if mapped.exact {
                        mapped.original.start += start - mapping.generated.start;
                        mapped.original.end = mapped.original.start + end - start;
                    }
                    mapped.generated = prefix + start - range.start..prefix + end - range.start;
                    Some(mapped)
                })
                .collect();
            let part = StylesheetPart {
                mappings,
                key: Fingerprint::of(&css),
                css,
                producers: owners,
                slot,
                sequence: *sequence,
            };
            *sequence += 1;
            part
        })
        .collect();
    Ok(parts)
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

    use stillwind_compiler::{CompilerOptions, Identifiers};
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
        let mut session = ProjectSession::new(
            snapshot,
            SessionMode::Watch,
            CompilerOptions::default(),
            Identifiers::builtin(),
        );
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
        let mut session = ProjectSession::new(
            snapshot,
            SessionMode::Build,
            CompilerOptions::default(),
            Identifiers::builtin(),
        );
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
