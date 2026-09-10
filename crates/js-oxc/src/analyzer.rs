use std::collections::BTreeSet;
use std::sync::Arc;

use stillwind_ir::{ModuleAnalysis, ModuleScan, SourceAnalyzer, TypeProbe};
use stillwind_semantic::{ExportedValue, IntegrationDiagnostic, TypeFacts};
use stillwind_source::{Fingerprint, SourceFile, collect_lint_directives};

use crate::parse_with_type_facts;

#[derive(Clone, Copy, Debug, Default)]
pub struct OxcSourceAnalyzer;

impl SourceAnalyzer for OxcSourceAnalyzer {
    fn scan(&self, source: Arc<SourceFile>) -> ModuleScan {
        let parsed = crate::parse(&source);
        ModuleScan {
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

    fn finalize(&self, scan: ModuleScan, facts: Option<&TypeFacts>) -> ModuleAnalysis {
        let mut integration_diagnostics = Vec::new();
        let facts = facts.filter(|facts| {
            let valid = facts.schema == TypeFacts::SCHEMA
                && facts.module_id == scan.module_id
                && facts.source_revision == scan.source_revision
                && facts.facts.iter().all(|fact| {
                    fact.start <= fact.end
                        && scan
                            .source
                            .text()
                            .get(fact.start as usize..fact.end as usize)
                            .is_some()
                });
            if !valid {
                integration_diagnostics.push(IntegrationDiagnostic {
                    module_id: Some(scan.module_id.clone()),
                    message: "incompatible, stale or invalid TypeScript facts".into(),
                });
            }
            valid
        });
        // Oxc arenas are request-local. Re-extract only when external facts exist;
        // neither scan nor finalized IR contains checker objects or module graphs.
        let parsed =
            facts.map(|facts| parse_with_type_facts(&scan.source, &facts.facts, &facts.assets));
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
            asset_support: Vec::new(),
            external_assets: facts.map_or_else(BTreeSet::new, |facts| {
                let mut assets = BTreeSet::new();
                for fact in &facts.facts {
                    match &fact.value {
                        ExportedValue::Refs(alternatives) => {
                            for alternative in alternatives {
                                assets.insert(alternative.reference.clone());
                                if let Some(scopes) = &alternative.permitted_scopes {
                                    assets.extend(
                                        scopes.finite_candidates().into_iter().flatten().cloned(),
                                    );
                                }
                            }
                        }
                        ExportedValue::Scopes(value)
                        | ExportedValue::Animations(value)
                        | ExportedValue::ViewTransitions(value) => {
                            assets.extend(value.finite_candidates().into_iter().flatten().cloned());
                        }
                    }
                }
                assets
            }),
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
