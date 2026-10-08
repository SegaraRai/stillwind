//! Plans CSS for producer modules selected by the host.
//!
//! [`StylesheetSelection`] names the producers for an output target. Only the host
//! knows whether it selected the whole module graph, so it must state the coverage.
//! [`ProjectSession::plan_stylesheet`] returns CSS with source mappings, resources,
//! and producer revisions. It rejects output kept after a failed design-system load.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{ParseError, Parser, Token};

use stillwind_compiler::{
    MappedStylesheet, ModuleArtifactKey, ModuleResourceReference, StylesheetMapping,
    StylesheetPlacement, StylesheetRenderer,
};
use stillwind_css::{CssRule, optimize_rules, stylesheet_fragments};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{FileId, Fingerprint, ModuleId, Span};

use crate::{ProjectSession, ProjectStylesheet};

/// Why a stylesheet plan could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum StylesheetPlanError {
    /// The selection names modules with no published artifact.
    #[error("Stillwind selection contains unregistered modules: {0:?}")]
    MissingModules(BTreeSet<ModuleId>),
    /// Complete coverage has no target or graph revision with non-whitespace text.
    #[error("Complete coverage requires a nonempty host target and graph revision")]
    MissingCoverageIdentity,
    /// A selected artifact no longer matches its analysis or design-system snapshot.
    #[error("Stillwind selection contains stale producers: {0:?}")]
    StaleModules(BTreeSet<ModuleId>),
    /// The latest load failed, so output kept from the previous revision cannot form a
    /// current plan.
    #[error("The latest design-system load failed; retained CSS is not a current output plan")]
    FailedDesignSystemLoad,
    /// Producer dependencies contain a cycle.
    #[error("Cyclic stylesheet producer dependencies: {0:?}")]
    CyclicModules(BTreeSet<ModuleId>),
    /// Rendering the selected CSS failed.
    #[error("{0}")]
    Stylesheet(#[from] CompileDiagnostic),
}

/// How much the host knows about the selected producer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StylesheetCoverage {
    /// Selects producers compiled so far. Other modules may still exist, so this
    /// selection cannot guarantee a complete stylesheet for the target.
    Observed,
    /// The host guarantees that the selection covers the target at this graph revision.
    ///
    /// Both fields must contain non-whitespace text. The plan keeps them so consumers
    /// can check which output and graph revision the guarantee applies to.
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

/// Contains rendered CSS, its placement, and source and producer information.
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

/// Contains one CSS fragment at a position in a stylesheet plan.
#[derive(Clone, Debug)]
pub struct StylesheetPart {
    /// Maps positions in the whole fragment, including its wrapper, to source
    /// locations.
    pub mappings: Vec<StylesheetMapping>,
    /// Identifies a node in the stylesheet template by its stable index.
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
    /// Returns a selection of published producers without claiming the graph is
    /// complete.
    pub fn observed_selection(&self) -> StylesheetSelection {
        StylesheetSelection {
            modules: self.artifacts().keys().cloned().collect(),
            coverage: StylesheetCoverage::Observed,
        }
    }

    /// Plans a module, chunk, entry, or document selection supplied by the host.
    ///
    /// `selection` supplies producers and their coverage. `placement` chooses document
    /// or carrier output. The plan includes composition dependencies and identifies
    /// each producer's artifact revision. The session remains unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if the latest design-system load failed, producers are missing
    /// or no longer match their analyses, dependencies contain a cycle, or CSS is
    /// invalid. Complete coverage requires target and revision strings with non
    /// whitespace text. Errors for missing or stale modules identify the modules the
    /// host must recompile.
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
        let stale = selection
            .modules
            .iter()
            .filter(
                |id| match (self.analyses().get(*id), self.artifacts().get(*id)) {
                    (Some(analysis), Some(_)) => {
                        !analysis.diagnostics.is_empty()
                            || !analysis.integration_diagnostics.is_empty()
                            || self.current_artifact(id).is_none()
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
                .find(|symbol| identifiers.contains(symbol.name.as_str()))?;
            // A name without a declaration, such as a conditional helper, has no position in the
            // module to map the rule back to.
            let (start, end) = symbol.start.zip(symbol.end)?;
            let source = std::iter::once(analysis.as_ref())
                .chain(analysis.asset_support.iter())
                .find(|input| symbol.origin.module_id() == Some(&input.module_id))?
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
        let source_modules: BTreeMap<FileId, ModuleId> = self
            .analyses()
            .values()
            .flat_map(|analysis| {
                std::iter::once(analysis.as_ref()).chain(analysis.asset_support.iter())
            })
            .map(|analysis| (analysis.source.id(), analysis.module_id.clone()))
            .collect();
        let parts = stylesheet_parts(&reference, &producers, &source_modules)?;
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
                    let base = dependency.base.module_id.clone();
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
                            edge.base.module_id == **module
                                || !remaining.contains(&edge.base.module_id)
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
        StylesheetPlanError::Stylesheet(CompileDiagnostic::InvalidGeneratedCss { offset })
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
    source_modules: &BTreeMap<FileId, ModuleId>,
) -> Result<Vec<StylesheetPart>, StylesheetPlanError> {
    let mut sequences = BTreeMap::new();
    let mappings = IntervalIndex::new(
        reference
            .mappings
            .iter()
            .map(|mapping| mapping.generated.clone()),
    );
    let segments = IntervalIndex::new(reference.segments.iter().map(|(range, _)| range.clone()));
    let parts = stylesheet_fragments(&reference.css)
        .map_err(|offset| {
            StylesheetPlanError::Stylesheet(CompileDiagnostic::InvalidGeneratedCss { offset })
        })?
        .into_iter()
        .map(|(css, range)| {
            let overlapping = mappings.overlapping(&range);
            let owners: BTreeSet<_> = overlapping
                .iter()
                .map(|index| &reference.mappings[*index])
                .filter_map(|mapping| source_modules.get(&mapping.source.id()).cloned())
                .filter(|id| producers.contains_key(id))
                .collect();
            let slot = segments
                .overlapping(&range)
                .into_iter()
                .map(|index| reference.segments[index].1)
                .min()
                .unwrap_or_default();
            let sequence = sequences.entry((slot, owners.clone())).or_insert(0);
            let prefix = css.rfind(&reference.css[range.clone()]).unwrap_or_default();
            let mappings = overlapping
                .into_iter()
                .map(|index| &reference.mappings[index])
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

/// Finds source mappings and owners for a CSS fragment.
/// Sorted starts and stored maximum ends let a query skip ranges that cannot match.
/// This avoids scanning every source range for each fragment.
struct IntervalIndex {
    /// CSS byte ranges are sorted by start and paired with their original input
    /// indices. Each range includes its start and excludes its end.
    ranges: Vec<(std::ops::Range<usize>, usize)>,
    /// Each entry records the largest end offset up to that position. A query can use
    /// binary search to skip earlier ranges even when ranges overlap or contain one
    /// another.
    maximum_ends: Vec<usize>,
}

impl IntervalIndex {
    /// Builds an index from byte ranges in the same CSS buffer. Input order is
    /// arbitrary. The index keeps each range's original position so callers can find
    /// its source data.
    fn new(ranges: impl Iterator<Item = std::ops::Range<usize>>) -> Self {
        let mut ranges: Vec<_> = ranges
            .enumerate()
            .map(|(index, range)| (range, index))
            .collect();
        ranges.sort_by_key(|(range, _)| range.start);
        let mut maximum = 0;
        let maximum_ends = ranges
            .iter()
            .map(|(range, _)| {
                maximum = maximum.max(range.end);
                maximum
            })
            .collect();
        Self {
            ranges,
            maximum_ends,
        }
    }

    /// Returns input indices for ranges that start before `query.end` and end after
    /// `query.start`. Results follow input order. `query` must be a nonempty CSS byte
    /// range.
    fn overlapping(&self, query: &std::ops::Range<usize>) -> Vec<usize> {
        let start = self.maximum_ends.partition_point(|end| *end <= query.start);
        let end = self
            .ranges
            .partition_point(|(range, _)| range.start < query.end);
        let mut indices: Vec<_> = self.ranges[start.min(end)..end]
            .iter()
            .filter(|(range, _)| range.end > query.start)
            .map(|(_, index)| *index)
            .collect();
        indices.sort_unstable();
        indices
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

    use stillwind_compiler::{CompilerOptions, Identifiers};
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::CompileError;
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, SourceFile, SourceRegistry};

    use super::*;
    use crate::SessionMode;

    #[test]
    fn retained_watch_output_cannot_be_reported_as_current_coverage() {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(
                FileId(0),
                None,
                None,
                "@utility red{color:red}@stillwind-slot utilities;",
            )
            .unwrap(),
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let mut session = ProjectSession::new(
            snapshot,
            &SourceRegistry::default(),
            SessionMode::Watch,
            CompilerOptions::default(),
            Identifiers::default(),
        );
        for utility in ["'red'", "unknown"] {
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                format!("import {{style}} from 'stillwind/react'; const x=style({utility});"),
            )
            .unwrap();
            let analysis = OxcSourceAnalyzer.finalize(
                OxcSourceAnalyzer.scan(
                    Arc::new(source),
                    ModuleId::from("test.tsx"),
                    ScriptLanguage::Tsx,
                    ScriptModuleKind::Unambiguous,
                ),
                None,
            );
            session.update_module(analysis);
        }
        assert!(session.css().unwrap().contains("color:red"));
        assert!(matches!(
            session.plan_stylesheet(session.observed_selection(), StylesheetPlacement::Document),
            Err(StylesheetPlanError::StaleModules(_))
        ));
    }

    #[test]
    fn interval_lookup_preserves_input_order_for_overlapping_and_unordered_ranges() {
        let ranges = [20..25, 0..100, 7..10, 5..9, 30..35];
        let index = IntervalIndex::new(ranges.iter().cloned());
        for query in [0..5, 6..8, 9..20, 21..22, 100..101] {
            let expected: Vec<_> = ranges
                .iter()
                .enumerate()
                .filter(|(_, range)| range.start < query.end && range.end > query.start)
                .map(|(index, _)| index)
                .collect();
            assert_eq!(index.overlapping(&query), expected);
        }
    }

    #[test]
    fn explicit_selection_excludes_observed_but_unselected_producers() {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(
                FileId(0),
                None,
                None,
                "body{margin:0}@utility red{color:red}@utility blue{color:blue}@stillwind-slot utilities;"
            ).unwrap(),
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        ).unwrap();
        let mut session = ProjectSession::new(
            snapshot,
            &SourceRegistry::default(),
            SessionMode::Build,
            CompilerOptions::default(),
            Identifiers::default(),
        );
        for (index, name) in ["red", "blue"].iter().enumerate() {
            let source = SourceFile::new(
                FileId(u32::try_from(index).unwrap() + 1),
                None,
                None,
                format!("import {{style}} from 'stillwind/react'; export const x=style('{name}');"),
            )
            .unwrap();
            let analysis = OxcSourceAnalyzer.finalize(
                OxcSourceAnalyzer.scan(
                    Arc::new(source),
                    ModuleId::from(format!("{name}.tsx").as_str()),
                    ScriptLanguage::Tsx,
                    ScriptModuleKind::Unambiguous,
                ),
                None,
            );
            assert_eq!(
                session.update_module(analysis).diagnostics,
                [] as [CompileError; 0]
            );
        }
        let observed = session
            .plan_stylesheet(session.observed_selection(), StylesheetPlacement::Document)
            .unwrap();
        assert_eq!(observed.coverage, StylesheetCoverage::Observed);
        assert!(observed.css.contains("color:red") && observed.css.contains("color:blue"));
        let selection = StylesheetSelection {
            modules: [ModuleId::from("red.tsx")].into(),
            coverage: StylesheetCoverage::Complete {
                target: "route/index".to_owned(),
                revision: "graph-2".to_owned(),
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
        session.remove_module(&ModuleId::from("red.tsx"));
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
