//! Single-module analysis and versioned optional TypeScript facts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_ir::ModuleIr;
use stillwind_semantic::{AssetMetadata, IdentityId, ModuleSummary, TypeFact};
use stillwind_source::{
    CompileError, LintDirective, LintFinding, SourceFile, Span, collect_lint_directives,
};

use crate::Fingerprint;

pub type ModuleId = String;

pub type SourceRevision = Fingerprint;

pub type SemanticRevision = Fingerprint;

pub type SourceDiagnostic = CompileError;

#[derive(Clone, Debug)]
pub struct TypeProbe {
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ModuleScan {
    pub module_id: ModuleId,
    pub source: Arc<SourceFile>,
    pub source_revision: SourceRevision,
    pub local_ir: ModuleIr,
    pub summary: ModuleSummary,
    pub type_probes: Vec<TypeProbe>,
    pub lint_directives: Vec<LintDirective>,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub lint_findings: Vec<LintFinding>,
}

#[derive(Clone, Debug)]
pub struct ModuleAnalysis {
    pub external_assets: BTreeSet<IdentityId>,
    pub module_id: ModuleId,
    pub source: Arc<SourceFile>,
    pub source_revision: SourceRevision,
    pub ir: ModuleIr,
    pub summary: ModuleSummary,
    pub lint_directives: Vec<LintDirective>,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub lint_findings: Vec<LintFinding>,
    pub semantic_revision: Option<SemanticRevision>,
    pub integration_diagnostics: Vec<IntegrationDiagnostic>,
    pub frontend_kind: String,
}

#[derive(Clone, Debug)]
pub struct TypeFacts {
    pub schema: u32,
    pub module_id: ModuleId,
    pub source_revision: SourceRevision,
    pub semantic_revision: SemanticRevision,
    pub facts: Vec<TypeFact>,
    pub assets: BTreeMap<IdentityId, AssetMetadata>,
}

impl TypeFacts {
    pub const SCHEMA: u32 = 1;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationDiagnostic {
    pub module_id: Option<ModuleId>,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct TypeFactsUpdate {
    pub modules: BTreeMap<ModuleId, TypeFacts>,
    pub diagnostics: Vec<IntegrationDiagnostic>,
}

pub trait TypeScriptFactsProvider {
    fn update(&mut self, scans: &[ModuleScan]) -> TypeFactsUpdate;
}

pub trait SourceAnalyzer {
    fn scan(&self, source: Arc<SourceFile>) -> ModuleScan;
    fn finalize(&self, scan: ModuleScan, facts: Option<&TypeFacts>) -> ModuleAnalysis;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OxcSourceAnalyzer;

impl SourceAnalyzer for OxcSourceAnalyzer {
    fn scan(&self, source: Arc<SourceFile>) -> ModuleScan {
        let parsed = stillwind_js_oxc::parse(&source);
        ModuleScan {
            module_id: source.path().to_owned(),
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
        let parsed = facts.map(|facts| {
            stillwind_js_oxc::parse_with_type_facts(&scan.source, &facts.facts, &facts.assets)
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
            external_assets: facts.map_or_else(BTreeSet::new, |facts| {
                let mut assets: BTreeSet<_> = facts.assets.keys().cloned().collect();
                for fact in &facts.facts {
                    match &fact.value {
                        stillwind_semantic::ExportedValue::Refs(alternatives) => {
                            for alternative in alternatives {
                                assets.insert(alternative.reference.clone());
                                if let Some(scopes) = &alternative.permitted_scopes {
                                    assets.extend(
                                        scopes.finite_candidates().into_iter().flatten().cloned(),
                                    );
                                }
                            }
                        }
                        stillwind_semantic::ExportedValue::Scopes(value)
                        | stillwind_semantic::ExportedValue::Animations(value)
                        | stillwind_semantic::ExportedValue::ViewTransitions(value) => {
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

impl ModuleAnalysis {
    pub(crate) fn parsed(&self) -> stillwind_js_oxc::ParsedJavaScript {
        stillwind_js_oxc::ParsedJavaScript {
            type_probes: Vec::new(),
            diagnostics: self.diagnostics.clone(),
            lint_findings: self.lint_findings.clone(),
            module: self.ir.clone(),
            summary: self.summary.clone(),
        }
    }
}
