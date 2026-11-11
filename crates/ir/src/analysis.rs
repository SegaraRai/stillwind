//! Defines how frontends scan source and produce module IR.
//!
//! Analysis has two steps because TypeScript facts can depend on several modules.
//! [`SourceAnalyzer::scan`] reads one module's local content. The host can then send
//! all scans to a [`TypeScriptFactsProvider`]. [`SourceAnalyzer::finalize`] uses the
//! results to produce IR for the compiler. Without facts, finalization supports only
//! a limited set of local cases. It reports values from other modules and non-trivial
//! conditionals as unsupported.

use std::collections::BTreeSet;
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_diagnostics::{CompileError, LintDirective, LintFinding};
use stillwind_semantic::{
    IdentityId, IntegrationDiagnostic, ModuleSummary, TypeFacts, TypeFactsUpdate,
};
use stillwind_source::{Fingerprint, ModuleId, SourceFile, SourceProvenance, Span};

use crate::{
    IncompatibleComposition, ModuleIr, ScriptLanguage, ScriptModuleKind, SourceInput,
    validate_composition,
};

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

/// The AST role required for a checker correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckerRole {
    /// An expression whose runtime type is queried.
    Expression,
    /// A call that receives a declaration identity argument.
    Producer,
    /// A string literal that identifies an imported module.
    ModuleSpecifier,
}

/// A validated generated location for one checker operation.
#[derive(Clone, Copy, Debug)]
pub struct CheckerSpan {
    /// Original and generated UTF-8 ranges.
    pub provenance: SourceProvenance,
    /// AST role of the generated range.
    pub role: CheckerRole,
}

/// A converter-created reference that must bind to its global helper declaration.
#[derive(Clone, Debug)]
pub struct CheckerBinding {
    /// Global helper identifier.
    pub name: LeanStr,
    /// Generated UTF-8 range of the identifier reference.
    pub span: Span,
}

/// Validated locations used to construct a projected checker source.
#[derive(Clone, Debug, Default)]
pub struct CheckerProjection {
    /// Correspondences for expressions, producers, and module specifiers.
    pub spans: Vec<CheckerSpan>,
    /// Generated callees that need independent helper bindings.
    pub bindings: Vec<CheckerBinding>,
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
    /// Checker operations validated against the projection map.
    pub checker_projection: CheckerProjection,
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

/// Supplies TypeScript facts to the frontend. Implementations follow dependencies,
/// manage the checker's lifetime, and invalidate outdated facts. They must keep
/// checker objects out of the analysis.
pub trait TypeScriptFactsProvider {
    /// Resolves probes from all `scans` in one batch and returns facts with their
    /// revisions. Implementations may omit facts they cannot prove.
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

/// Contains finalized module IR and metadata used by the compiler and project session.
#[derive(Clone, Debug)]
pub struct ModuleAnalysis {
    /// The input this analysis was produced from.
    pub input: SourceInput,
    /// Projection provenance, empty when the input had no projection.
    pub provenance: Vec<SourceProvenance>,
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
    /// Lint findings. Their presence, severity, and suppression never change generated
    /// names or output.
    pub lint_findings: Vec<LintFinding>,
    /// Fingerprint of the facts used, or `None` when analysis ran without them.
    pub semantic_revision: Option<Fingerprint>,
    /// Problems reported by the host's fact provider, such as a missing manifest for a
    /// package whose asset semantics are required.
    pub integration_diagnostics: Vec<IntegrationDiagnostic>,
    /// Frontend implementation name, part of artifact identity.
    pub frontend_kind: LeanStr,
}

impl ModuleAnalysis {
    /// Reports incompatible composition bases and invalid composition graphs.
    ///
    /// `inputs` must contain the complete producer graph with each module identity once.
    /// Returns cloned analyses in input order, or an empty vector for empty input. The
    /// clones keep their source coordinates. A composition operand that can supply a
    /// base with inline style adds a lint finding. Missing producers and cycles add
    /// fatal errors at each module's full source span.
    ///
    /// This method checks the graph once for the whole batch instead of once per module.
    /// It leaves `inputs` unchanged.
    pub fn validate_composition_batch(inputs: &[Self]) -> Vec<Self> {
        let incompatible = IncompatibleComposition::find(&inputs.iter().collect::<Vec<_>>());
        let mut lowered = inputs.to_vec();
        for analysis in &mut lowered {
            let findings = incompatible.findings(analysis);
            analysis.lint_findings.extend(findings);
        }
        let modules: Vec<_> = lowered.iter().collect();
        let diagnostics = inputs.first().map_or_else(Vec::new, |input| {
            validate_composition(&modules, input.source.full_span(), true)
        });
        lowered
            .into_iter()
            .map(|mut analysis| {
                for error in &diagnostics {
                    analysis.diagnostics.push(CompileError::new(
                        error.diagnostic.clone(),
                        analysis.source.full_span(),
                    ));
                }
                analysis
            })
            .collect()
    }
}
