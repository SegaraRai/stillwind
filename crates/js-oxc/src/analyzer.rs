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
//! a failed projection becomes a projection diagnostic on a best-effort scan, and
//! unusable facts become an [`IntegrationDiagnostic`] while the scan's own result is
//! used unchanged.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lean_string::ToLeanStr;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{
    CheckerProjection, ModuleAnalysis, ModuleIr, ModuleScan, SourceAnalyzer, SourceInput, TypeProbe,
};
use stillwind_semantic::{
    ExportedValue, IdentityId, IntegrationDiagnostic, ModuleSummary, TypeFacts,
};
use stillwind_source::{Fingerprint, JoinText};

use crate::ParsedJavaScript;
use crate::entry::parse_with_lint;
use crate::projected_scan::parse_projected;

/// Oxc-backed analyser. Holds no state, because Oxc arenas are request-local.
///
/// Reusing one value across modules is expected; anything module-specific lives in the
/// [`ModuleScan`] it returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct OxcSourceAnalyzer;

impl OxcSourceAnalyzer {
    /// Selects whether scanning and finalization collect advisory source findings.
    /// Fatal diagnostics and the extracted IR do not depend on `lint`.
    /// Use the returned analyzer for both scanning and finalization.
    pub fn with_lint(lint: bool) -> impl SourceAnalyzer {
        ConfiguredSourceAnalyzer { lint }
    }

    /// The asset identities another module's accepted facts can reach.
    ///
    /// Finite candidates become stylesheet roots. Universe values contribute no
    /// identities because the checker did not determine their declarations.
    fn external_assets(facts: Option<&TypeFacts>) -> BTreeSet<IdentityId> {
        let Some(facts) = facts else {
            return BTreeSet::new();
        };
        let mut assets = BTreeSet::new();
        for fact in &facts.facts {
            let stillwind_semantic::CheckedValue::Asset(value) = &fact.value else {
                continue;
            };
            match value {
                ExportedValue::Styles(value)
                | ExportedValue::Refs(value)
                | ExportedValue::Keyframes(value)
                | ExportedValue::ViewTransitions(value) => {
                    assets.extend(value.finite_candidates().into_iter().flatten().cloned());
                }
            }
        }
        assets
    }
}

impl SourceAnalyzer for OxcSourceAnalyzer {
    fn scan_input(&self, input: SourceInput) -> ModuleScan {
        Self::with_lint(true).scan_input(input)
    }

    fn finalize(&self, scan: ModuleScan, facts: Option<&TypeFacts>) -> ModuleAnalysis {
        Self::with_lint(true).finalize(scan, facts)
    }
}

/// Frontend with a fixed policy for advisory source checks.
struct ConfiguredSourceAnalyzer {
    /// Whether scanning and fact-based extraction collect advisory findings.
    lint: bool,
}

impl SourceAnalyzer for ConfiguredSourceAnalyzer {
    /// Parses the editable source and returns IR, diagnostics and a summary.
    ///
    /// A projected input is normalized and mapped back to original spans before
    /// the IR is produced, so every span in the result belongs to the original
    /// file. Projection failures do not propagate as errors: the scan carries
    /// a projection diagnostic and empty IR so the caller can report and continue.
    fn scan_input(&self, input: SourceInput) -> ModuleScan {
        let source = Arc::clone(input.document().file());
        let (mut parsed, provenance, checker_projection) =
            if let Some(projected) = input.projected() {
                match parse_projected(
                    &input.module_id,
                    projected,
                    &[],
                    &BTreeMap::new(),
                    self.lint,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        return ModuleScan {
                            module_id: input.module_id.clone(),
                            source_revision: Fingerprint::of(source.text()),
                            local_ir: ModuleIr::react(),
                            summary: ModuleSummary::new(input.module_id.clone()),
                            type_probes: Vec::new(),
                            lint_directives: Vec::new(),
                            diagnostics: vec![error],
                            lint_findings: Vec::new(),
                            input,
                            provenance: Vec::new(),
                            checker_projection: CheckerProjection::default(),
                            source,
                        };
                    }
                }
            } else {
                (
                    parse_with_lint(
                        &source,
                        &input.module_id,
                        &[],
                        &BTreeMap::new(),
                        input.syntax().language(),
                        input.syntax().module_kind(),
                        self.lint,
                    ),
                    Vec::new(),
                    CheckerProjection::default(),
                )
            };
        select_framework(&mut parsed, &input, self.lint);
        let module_id = input.module_id.clone();
        ModuleScan {
            input,
            provenance,
            checker_projection,
            module_id,
            source_revision: Fingerprint::of(source.text()),
            type_probes: parsed
                .type_probes
                .into_iter()
                .map(|span| TypeProbe {
                    span,
                    recognition_only: false,
                })
                .chain(parsed.use_probes.into_iter().map(|span| TypeProbe {
                    span,
                    recognition_only: true,
                }))
                .collect(),
            lint_directives: parsed.lint_directives,
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
                    diagnostic: CompileDiagnostic::InvalidTypeScriptFacts,
                });
            }
            valid
        });
        // Oxc arenas are request-local. Re-extract only when external facts exist;
        // neither scan nor finalized IR contains checker objects or module graphs.
        let mut provenance = scan.provenance;
        let parsed = facts.map(|facts| {
            let mut parsed = if let Some(projected) = scan.input.projected() {
                match parse_projected(
                    &scan.module_id,
                    projected,
                    &facts.facts,
                    &facts.assets,
                    self.lint,
                ) {
                    Ok((parsed, mapped, _)) => {
                        provenance = mapped;
                        parsed
                    }
                    Err(error) => {
                        let mut parsed = ParsedJavaScript {
                            use_probes: Vec::new(),
                            framework: None,
                            type_probes: Vec::new(),
                            diagnostics: vec![error],
                            lint_findings: Vec::new(),
                            module: ModuleIr::react(),
                            summary: ModuleSummary::new(scan.module_id.clone()),
                            lint_directives: Vec::new(),
                        };
                        select_framework(&mut parsed, &scan.input, self.lint);
                        return parsed;
                    }
                }
            } else {
                parse_with_lint(
                    &scan.source,
                    &scan.module_id,
                    &facts.facts,
                    &facts.assets,
                    scan.input.syntax().language(),
                    scan.input.syntax().module_kind(),
                    self.lint,
                )
            };
            select_framework(&mut parsed, &scan.input, self.lint);
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
            external_assets: OxcSourceAnalyzer::external_assets(facts),
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

/// Checks macro imports against the original format and selects the output adapter.
///
/// Format mismatches append advisory findings at import literals. An explicit adapter
/// that contradicts the macro import appends
/// [`CompileDiagnostic::FrameworkConstraintConflict`]; otherwise it overrides the
/// default adapter. Component formats supply the default when no macro is imported.
fn select_framework(parsed: &mut ParsedJavaScript, input: &SourceInput, lint: bool) {
    let allowed = input.document().format().adapters();
    for import in &parsed.module.imports.macros {
        if lint && !allowed.is_empty() && !allowed.contains(&import.adapter) {
            parsed.lint_findings.push(LintFinding::new(
                LintDiagnostic::MismatchedFrameworkImport {
                    import: import.adapter.spec().macro_import.into(),
                    source_format: input.document().format().description().into(),
                    expected: (allowed
                        .iter()
                        .map(|adapter| {
                            format_args!("`{}`", adapter.spec().macro_import).to_lean_str()
                        })
                        .collect::<Vec<_>>()
                        .join_text(", "))
                    .into(),
                },
                import.source_span,
            ));
        }
    }
    if let Some(framework) = input.framework {
        if parsed
            .framework
            .is_some_and(|selected| selected != framework)
        {
            parsed.diagnostics.push(CompileError::new(
                CompileDiagnostic::FrameworkConstraintConflict,
                input.document().file().full_span(),
            ));
        } else {
            parsed.module.adapter = framework;
        }
    } else if parsed.framework.is_none()
        && let Some(framework) = input.document().format().projection_adapter()
    {
        parsed.module.adapter = framework;
    }
}

#[cfg(test)]
mod tests {
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_semantic::{AssetKind, SymbolicValue, TypeFact};
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    #[test]
    fn disabling_source_lint_preserves_extraction_and_fatal_diagnostics() {
        let source = Arc::new(SourceFile::new(FileId(1), None, None,
            "import {style} from 'stillwind/react'; let state = 0; export const props = style({'--x': state++});"
        ).unwrap());
        for lint in [true, false] {
            let analyzer = OxcSourceAnalyzer::with_lint(lint);
            let scan = analyzer.scan(
                source.clone(),
                "entry.ts".into(),
                ScriptLanguage::Ts,
                ScriptModuleKind::Module,
            );
            assert_eq!(scan.local_ir.style_calls.len(), 1);
            assert_eq!(scan.lint_findings.is_empty(), !lint);
            let facts = TypeFacts {
                schema: TypeFacts::SCHEMA,
                module_id: scan.module_id.clone(),
                source_revision: scan.source_revision.clone(),
                semantic_revision: Fingerprint::of("facts"),
                facts: Vec::new(),
                assets: BTreeMap::new(),
            };
            let analysis = analyzer.finalize(scan, Some(&facts));
            assert_eq!(analysis.ir.style_calls.len(), 1);
            assert_eq!(analysis.lint_findings.is_empty(), !lint);
        }
        let invalid = Arc::new(SourceFile::new(FileId(2), None, None, "export const =;").unwrap());
        let scan = OxcSourceAnalyzer::with_lint(false).scan(
            invalid,
            "invalid.ts".into(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        assert_eq!(
            scan.diagnostics[0].diagnostic,
            CompileDiagnostic::JavaScriptParse
        );
    }

    #[test]
    fn analysis_accepts_current_schema_and_rejects_older_facts() {
        let source = Arc::new(SourceFile::new(FileId(1), None, None, "export {};").unwrap());
        let scan = OxcSourceAnalyzer.scan(
            source,
            "consumer.ts".into(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        assert_eq!(scan.summary.schema, 4);
        for (schema, accepted) in [(3, false), (4, false), (TypeFacts::SCHEMA, true)] {
            let facts = TypeFacts {
                schema,
                module_id: scan.module_id.clone(),
                source_revision: scan.source_revision.clone(),
                semantic_revision: Fingerprint::of("facts"),
                facts: Vec::new(),
                assets: BTreeMap::new(),
            };
            let analysis = OxcSourceAnalyzer.finalize(scan.clone(), Some(&facts));
            if accepted {
                assert_eq!(analysis.semantic_revision, Some(facts.semantic_revision));
                assert_eq!(
                    analysis.integration_diagnostics,
                    [] as [IntegrationDiagnostic; 0]
                );
            } else {
                assert!(analysis.semantic_revision.is_none());
                assert_eq!(analysis.ir, scan.local_ir);
                assert_eq!(analysis.integration_diagnostics.len(), 1);
                assert_eq!(
                    analysis.integration_diagnostics[0].diagnostic,
                    CompileDiagnostic::InvalidTypeScriptFacts,
                );
            }
        }
    }

    #[test]
    fn external_ref_assets_include_only_finite_candidates() {
        let reference = IdentityId::new(AssetKind::Ref, "refs.ts".into(), 0, 1, "handle");
        let facts = TypeFacts {
            schema: TypeFacts::SCHEMA,
            module_id: "consumer.ts".into(),
            source_revision: Fingerprint::of(""),
            semantic_revision: Fingerprint::of("facts"),
            facts: vec![
                TypeFact {
                    start: 0,
                    end: 0,
                    value: ExportedValue::Refs(SymbolicValue::exact(reference.clone())).into(),
                },
                TypeFact {
                    start: 0,
                    end: 0,
                    value: ExportedValue::Refs(SymbolicValue::universe(AssetKind::Ref)).into(),
                },
            ],
            assets: BTreeMap::new(),
        };
        assert_eq!(
            OxcSourceAnalyzer::external_assets(Some(&facts)),
            BTreeSet::from([reference])
        );
        assert!(OxcSourceAnalyzer::external_assets(None).is_empty());
    }
}
