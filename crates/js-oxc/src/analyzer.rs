//! The [`SourceAnalyzer`] implementation that ties scanning and facts together.
//!
//! [`SourceAnalyzer::scan_input`] is the cheap pass: it parses the edit source,
//! projects when the input carries a converter map, and returns IR plus a module
//! summary without any type checker. [`SourceAnalyzer::finalize`] is the expensive
//! pass: it validates optional [`TypeFacts`] against the scan and re-parses with them
//! when they are usable.
//!
//! The split exists so a host can cache scans across edits and only pay for
//! finalization when the checker's answers change. Neither pass may panic on bad input:
//! a failed projection becomes an `SHPRJ001` diagnostic on a best-effort scan, and
//! unusable facts become an [`IntegrationDiagnostic`] while the scan's own result is
//! used unchanged.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_diagnostics::{CompileError, CompileErrorKind, collect_lint_directives};
use stillwind_ir::{ModuleAnalysis, ModuleIr, ModuleScan, SourceAnalyzer, SourceInput, TypeProbe};
use stillwind_semantic::{
    ExportedValue, IntegrationDiagnostic, ModuleSummary, SymbolicValue, TypeFacts,
};
use stillwind_source::Fingerprint;

use crate::projected_scan::parse_projected;
use crate::{ParsedJavaScript, parse_with_language};

/// Oxc-backed analyser. Holds no state, because Oxc arenas are request-local.
///
/// Reusing one value across modules is expected; anything module-specific lives in the
/// [`ModuleScan`] it returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct OxcSourceAnalyzer;

impl OxcSourceAnalyzer {
    /// The asset identities another module's accepted facts can reach.
    ///
    /// A bounded ref keeps its boundary reachable, so the boundary's CSS stays in the
    /// stylesheet even when the declaration itself lives in another module.
    ///
    /// This is an associated function rather than a method: it reads facts alone, and
    /// finalization is the only caller.
    fn external_assets(facts: Option<&TypeFacts>) -> BTreeSet<stillwind_semantic::IdentityId> {
        let Some(facts) = facts else {
            return BTreeSet::new();
        };
        let mut assets = BTreeSet::new();
        for fact in &facts.facts {
            match &fact.value {
                ExportedValue::Refs(alternatives) => {
                    for alternative in alternatives {
                        assets.insert(alternative.reference.clone());
                        assets.extend(
                            alternative
                                .permitted_boundaries
                                .iter()
                                .filter_map(SymbolicValue::finite_candidates)
                                .flatten()
                                .cloned(),
                        );
                    }
                }
                ExportedValue::Styles(value)
                | ExportedValue::Keyframes(value)
                | ExportedValue::ViewTransitions(value)
                | ExportedValue::ViewTransitionKeys(value) => {
                    assets.extend(value.finite_candidates().into_iter().flatten().cloned());
                }
            }
        }
        assets
    }
}

impl SourceAnalyzer for OxcSourceAnalyzer {
    /// Parses the editable source and returns IR, diagnostics and a summary.
    ///
    /// A projected input is normalized and mapped back to original spans before
    /// the IR is produced, so every span in the result belongs to the original
    /// file. Projection failures do not propagate as errors: the scan carries an
    /// `SHPRJ001` diagnostic and empty IR so the caller can report and continue.
    fn scan_input(&self, input: SourceInput) -> ModuleScan {
        let source = Arc::clone(input.edit_source());
        let (mut parsed, provenance) = if input.projection.is_some() {
            match parse_projected(&input, &[], &BTreeMap::new()) {
                Ok(value) => value,
                Err(error) => {
                    return ModuleScan {
                        module_id: source.path().into(),
                        source_revision: Fingerprint::of(source.text()),
                        local_ir: ModuleIr::react(),
                        summary: ModuleSummary::new(source.path()),
                        type_probes: Vec::new(),
                        lint_directives: collect_lint_directives(source.text()),
                        diagnostics: vec![CompileError::new(error.into(), source.full_span())],
                        lint_findings: Vec::new(),
                        input,
                        provenance: Vec::new(),
                        source,
                    };
                }
            }
        } else {
            (
                parse_with_language(&source, &[], &BTreeMap::new(), input.language),
                Vec::new(),
            )
        };
        select_framework(&mut parsed, &input);
        ModuleScan {
            input,
            provenance,
            module_id: source.path().into(),
            source_revision: Fingerprint::of(source.text()),
            type_probes: parsed
                .type_probes
                .into_iter()
                .map(|span| TypeProbe { span })
                .collect(),
            lint_directives: collect_lint_directives(source.text()),
            source,
            local_ir: parsed.module,
            summary: parsed.summary,
            diagnostics: parsed.diagnostics,
            lint_findings: parsed.lint_findings,
        }
    }

    /// Applies checker facts to a scan, returning the analysis to cache.
    ///
    /// Facts are used only when the schema, module id and source revision all match and
    /// every fact span slices the scanned source; otherwise the scan's own IR is kept
    /// and an [`IntegrationDiagnostic`] records why. Projected facts use original UTF-8
    /// ranges through the same normalized analysis mapping as the scan.
    /// [`ModuleAnalysis::semantic_revision`] is copied from accepted facts and is
    /// `None` when they were rejected.
    fn finalize(&self, scan: ModuleScan, facts: Option<&TypeFacts>) -> ModuleAnalysis {
        let mut integration_diagnostics = Vec::new();
        let facts = facts.filter(|facts| {
            let valid = facts.schema == TypeFacts::SCHEMA
                && facts.module_id == scan.module_id
                && facts.source_revision == scan.source_revision
                && facts.facts.iter().all(|fact| {
                    fact.start <= fact.end
                        && fact.value.validate().is_ok()
                        && scan
                            .source
                            .text()
                            .get(fact.start as usize..fact.end as usize)
                            .is_some()
                });
            if !valid {
                integration_diagnostics.push(IntegrationDiagnostic {
                    module_id: Some(scan.module_id.clone()),
                    kind: CompileErrorKind::InvalidTypeScriptFacts,
                });
            }
            valid
        });
        // Oxc arenas are request-local. Re-extract only when external facts exist;
        // neither scan nor finalized IR contains checker objects or module graphs.
        let mut provenance = scan.provenance;
        let parsed = facts.map(|facts| {
            let mut parsed = if scan.input.projection.is_some() {
                match parse_projected(&scan.input, &facts.facts, &facts.assets) {
                    Ok((parsed, mapped)) => {
                        provenance = mapped;
                        parsed
                    }
                    Err(error) => {
                        let mut parsed = ParsedJavaScript {
                            framework: None,
                            type_probes: Vec::new(),
                            diagnostics: vec![CompileError::new(
                                error.into(),
                                scan.source.full_span(),
                            )],
                            lint_findings: Vec::new(),
                            module: ModuleIr::react(),
                            summary: ModuleSummary::new(scan.source.path()),
                        };
                        select_framework(&mut parsed, &scan.input);
                        return parsed;
                    }
                }
            } else {
                parse_with_language(
                    &scan.source,
                    &facts.facts,
                    &facts.assets,
                    scan.input.language,
                )
            };
            select_framework(&mut parsed, &scan.input);
            parsed
        });
        let (ir, summary, diagnostics, lint_findings) = parsed.map_or(
            (
                scan.local_ir,
                scan.summary,
                scan.diagnostics,
                scan.lint_findings,
            ),
            |parsed| {
                (
                    parsed.module,
                    parsed.summary,
                    parsed.diagnostics,
                    parsed.lint_findings,
                )
            },
        );
        ModuleAnalysis {
            input: scan.input,
            provenance,
            asset_support: Vec::new(),
            external_assets: Self::external_assets(facts),
            module_id: scan.module_id,
            source: scan.source,
            source_revision: scan.source_revision,
            ir,
            summary,
            lint_directives: scan.lint_directives,
            diagnostics,
            lint_findings,
            semantic_revision: facts.map(|facts| facts.semantic_revision.clone()),
            integration_diagnostics,
            frontend_kind: "oxc".into(),
        }
    }
}

/// Applies an explicitly requested framework to a parsed module.
///
/// A request that contradicts the macro import is reported as `SHJS008` instead of
/// silently overriding either side.
fn select_framework(parsed: &mut ParsedJavaScript, input: &SourceInput) {
    if let Some(framework) = input.framework {
        if parsed
            .framework
            .is_some_and(|selected| selected != framework)
        {
            parsed.diagnostics.push(CompileError::new(
                CompileErrorKind::FrameworkConflict,
                input.edit_source().full_span(),
            ));
        } else {
            parsed.module.adapter = framework;
        }
    }
}
