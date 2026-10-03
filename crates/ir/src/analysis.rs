//! The two-phase analysis contract every frontend implements.
//!
//! Analysis runs in two steps because TypeScript facts are computed across modules:
//! [`SourceAnalyzer::scan`] extracts what one module contains locally, the host may
//! then feed all scans to a [`TypeScriptFactsProvider`], and
//! [`SourceAnalyzer::finalize`] produces the IR the compiler consumes. Without facts,
//! [`SourceAnalyzer::finalize`] must support only the intentionally limited local
//! cases; cross-module values and non-trivial conditionals are reported as unsupported
//! rather than approximated in Rust.

use std::collections::BTreeSet;
use std::sync::Arc;

use stillwind_diagnostics::{CompileError, LintDirective, LintFinding};
use stillwind_semantic::{
    AssetKind, IdentityId, IntegrationDiagnostic, ModuleSummary, TypeFacts, TypeFactsUpdate,
};
use stillwind_source::{Fingerprint, ModuleId, SourceFile, SourceProvenance, Span};

use crate::{ModuleIr, ScriptLanguage, ScriptModuleKind, SourceInput, validate_composition};

/// An expression whose TypeScript type the host queries for lowering or macro
/// recognition.
#[derive(Clone, Debug)]
pub struct TypeProbe {
    /// Original UTF-8 span of the expression to query.
    pub span: Span,
    /// True when the query only identifies whether a receiver is a Stillwind macro
    /// asset. Resolving a fact is optional: without one, the receiver is treated as
    /// ordinary JavaScript and this query produces no error. Recognized assets can be
    /// used to diagnose unconsumed macro syntax.
    pub recognition_only: bool,
}

/// Local extraction for one module, before cross-module facts exist.
///
/// A scan is a valid input for its own [`SourceAnalyzer::finalize`] call. Its spans
/// refer to `source`, which is the editable original, while `input` records how that
/// source was obtained.
#[derive(Clone, Debug)]
pub struct ModuleScan {
    /// The input this scan was produced from, including its projection.
    pub input: SourceInput,
    /// Projection provenance, empty when the input had no projection.
    pub provenance: Vec<SourceProvenance>,
    /// Identity of the module being analyzed.
    pub module_id: ModuleId,
    /// Editable source the spans refer to.
    pub source: Arc<SourceFile>,
    /// Content revision of `source`, part of the artifact identity.
    pub source_revision: Fingerprint,
    /// IR built from local information only.
    pub local_ir: ModuleIr,
    /// Producer identities and metadata other modules may consume.
    pub summary: ModuleSummary,
    /// Expressions the host should resolve through the TypeScript checker.
    pub type_probes: Vec<TypeProbe>,
    /// Suppression directives whose scope may still cover later findings.
    pub lint_directives: Vec<LintDirective>,
    /// Scan-time errors. Finalization with accepted facts may replace these with
    /// diagnostics from a new parse.
    pub diagnostics: Vec<CompileError>,
    /// Findings that do not depend on cross-module facts.
    pub lint_findings: Vec<LintFinding>,
}

/// Host-side TypeScript integration. Implementations own dependency traversal, checker
/// lifetime, and invalidation; they must not leak checker objects into the analysis.
pub trait TypeScriptFactsProvider {
    /// Resolves the probes of every scan in one batch and returns versioned facts.
    /// Implementations may omit facts they cannot prove.
    fn update(&mut self, scans: &[ModuleScan]) -> TypeFactsUpdate;
}

/// Frontend entry point used by every host.
pub trait SourceAnalyzer {
    /// Scans a plain source buffer with no projection.
    fn scan(
        &self,
        source: Arc<SourceFile>,
        module_id: ModuleId,
        language: ScriptLanguage,
        module_kind: ScriptModuleKind,
    ) -> ModuleScan {
        self.scan_input(SourceInput::normal(
            source,
            module_id,
            language,
            module_kind,
        ))
    }

    /// Scans one host-provided input, projection included.
    fn scan_input(&self, input: SourceInput) -> ModuleScan;

    /// Produces the final analysis. `facts` is present only after the host has run its
    /// provider over all scans.
    fn finalize(&self, scan: ModuleScan, facts: Option<&TypeFacts>) -> ModuleAnalysis;
}

/// Finalized module IR plus everything the compiler and project layer need.
#[derive(Clone, Debug)]
pub struct ModuleAnalysis {
    /// The input this analysis was produced from.
    pub input: SourceInput,
    /// Projection provenance, empty when the input had no projection.
    pub provenance: Vec<SourceProvenance>,
    /// Declaration-only semantic inputs needed by this producer's standalone CSS.
    ///
    /// These are attached by [`ModuleAnalysis::attach_asset_support`] and are not
    /// dependencies for invalidation: they exist so a single module can be rendered in
    /// isolation, and they never add producers to a project.
    pub asset_support: Vec<ModuleAnalysis>,
    /// Assets this module consumes from elsewhere. A module may only use assets it
    /// declares itself or that appear here.
    pub external_assets: BTreeSet<IdentityId>,
    /// Identity of the module being analyzed.
    pub module_id: ModuleId,
    /// Editable source the spans refer to; the target of source transforms.
    pub source: Arc<SourceFile>,
    /// Content revision of `source`, part of the artifact identity.
    pub source_revision: Fingerprint,
    /// Final IR, after any fact-driven decisions.
    pub ir: ModuleIr,
    /// Producer identities and metadata other modules may consume.
    pub summary: ModuleSummary,
    /// Suppression directives whose scope may still cover later findings.
    pub lint_directives: Vec<LintDirective>,
    /// Final errors. Compilation must not produce an artifact when this is non-empty.
    pub diagnostics: Vec<CompileError>,
    /// Lint findings. They never change generated names or output.
    pub lint_findings: Vec<LintFinding>,
    /// Fingerprint of the facts used, or `None` when analysis ran without them.
    pub semantic_revision: Option<Fingerprint>,
    /// Problems reported by the host's fact provider, such as a missing manifest for a
    /// package whose asset semantics are required.
    pub integration_diagnostics: Vec<IntegrationDiagnostic>,
    /// Frontend implementation name, part of artifact identity.
    pub frontend_kind: String,
}

impl ModuleAnalysis {
    /// Validates a complete producer graph once and attaches referenced declaration CSS.
    /// `inputs` must contain each module identity once. Returned analyses retain their
    /// source coordinates and receive graph errors at their own full source span.
    pub fn attach_asset_support_batch(inputs: &[Self]) -> Vec<Self> {
        let modules: Vec<_> = inputs.iter().collect();
        let diagnostics = inputs.first().map_or_else(Vec::new, |input| {
            validate_composition(&modules, input.source.full_span(), true)
        });
        inputs
            .iter()
            .map(|input| {
                let mut analysis = input.clone();
                for error in &diagnostics {
                    analysis.diagnostics.push(CompileError::new(
                        error.diagnostic.clone(),
                        input.source.full_span(),
                    ));
                }
                analysis.attach_declarations(inputs);
                analysis
            })
            .collect()
    }

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
        self.attach_declarations(inputs);
    }

    fn attach_declarations(&mut self, inputs: &[Self]) {
        self.asset_support.clear();
        if self.external_assets.is_empty() {
            return;
        }
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
            support.ir.ref_definitions.clear();
            support.ir.keyframes_declarations.retain(|declaration| {
                identities.iter().any(|identity| {
                    identity.kind == AssetKind::Keyframes
                        && identity.binding_name == declaration.binding.value
                })
            });
            support
                .ir
                .view_transition_declarations
                .retain(|declaration| {
                    identities.iter().any(|identity| {
                        identity.kind == AssetKind::ViewTransition
                            && identity.binding_name == declaration.binding.value
                    })
                });
            support
                .summary
                .assets
                .retain(|asset| identities.contains(&asset.identity));
            if !support.ir.keyframes_declarations.is_empty()
                || !support.ir.view_transition_declarations.is_empty()
            {
                self.asset_support.push(support);
            }
        }
    }
}
