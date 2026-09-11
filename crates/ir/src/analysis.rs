use std::collections::BTreeSet;
use std::sync::Arc;

use stillwind_semantic::{
    AssetKind, IdentityId, IntegrationDiagnostic, ModuleId, ModuleSummary, TypeFacts,
    TypeFactsUpdate,
};
use stillwind_source::{
    CompileError, Fingerprint, LintDirective, LintFinding, SourceFile, SourceProvenance, Span,
};

use crate::{ModuleIr, SourceInput, validate_composition};

#[derive(Clone, Debug)]
pub struct TypeProbe {
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ModuleScan {
    pub input: SourceInput,
    pub provenance: Vec<SourceProvenance>,
    pub module_id: ModuleId,
    pub source: Arc<SourceFile>,
    pub source_revision: Fingerprint,
    pub local_ir: ModuleIr,
    pub summary: ModuleSummary,
    pub type_probes: Vec<TypeProbe>,
    pub lint_directives: Vec<LintDirective>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
}

#[derive(Clone, Debug)]
pub struct ModuleAnalysis {
    pub input: SourceInput,
    pub provenance: Vec<SourceProvenance>,
    /// Declaration-only semantic inputs needed by this producer's standalone CSS.
    pub asset_support: Vec<ModuleAnalysis>,
    pub external_assets: BTreeSet<IdentityId>,
    pub module_id: ModuleId,
    pub source: Arc<SourceFile>,
    pub source_revision: Fingerprint,
    pub ir: ModuleIr,
    pub summary: ModuleSummary,
    pub lint_directives: Vec<LintDirective>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub semantic_revision: Option<Fingerprint>,
    pub integration_diagnostics: Vec<IntegrationDiagnostic>,
    pub frontend_kind: String,
}

pub trait TypeScriptFactsProvider {
    fn update(&mut self, scans: &[ModuleScan]) -> TypeFactsUpdate;
}

pub trait SourceAnalyzer {
    fn scan(&self, source: Arc<SourceFile>) -> ModuleScan {
        self.scan_input(SourceInput::normal(source))
    }
    fn scan_input(&self, input: SourceInput) -> ModuleScan;
    fn finalize(&self, scan: ModuleScan, facts: Option<&TypeFacts>) -> ModuleAnalysis;
}

impl ModuleAnalysis {
    /// Attaches referenced declaration CSS without registering semantic inputs as producers.
    pub fn attach_asset_support(&mut self, inputs: &[Self]) {
        let mut modules = vec![&*self];
        modules.extend(
            inputs
                .iter()
                .filter(|input| input.module_id != self.module_id),
        );
        let diagnostics = validate_composition(&modules, self.source.full_span(), true);
        self.diagnostics.extend(diagnostics);
        self.asset_support.clear();
        for input in inputs {
            if input.module_id == self.module_id {
                continue;
            }
            let identities: BTreeSet<_> = input
                .summary
                .assets
                .iter()
                .filter(|asset| self.external_assets.contains(&asset.identity))
                .map(|asset| asset.identity.clone())
                .collect();
            if identities.is_empty() {
                continue;
            }
            let mut support = input.clone();
            support.asset_support.clear();
            support.ir.style_calls.clear();
            support.ir.handle_definitions.clear();
            support.ir.scope_definitions.clear();
            support.ir.animation_declarations.retain(|declaration| {
                identities.iter().any(|identity| {
                    identity.kind == AssetKind::Animation
                        && identity.binding_name == declaration.name.value
                })
            });
            support
                .ir
                .view_transition_declarations
                .retain(|declaration| {
                    identities.iter().any(|identity| {
                        identity.kind == AssetKind::ViewTransition
                            && identity.binding_name == declaration.name.value
                    })
                });
            support
                .summary
                .assets
                .retain(|asset| identities.contains(&asset.identity));
            if !support.ir.animation_declarations.is_empty()
                || !support.ir.view_transition_declarations.is_empty()
            {
                self.asset_support.push(support);
            }
        }
    }
}
