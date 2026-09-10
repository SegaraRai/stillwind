//! Single-module analysis and versioned optional TypeScript facts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_ir::ModuleIr;
use stillwind_semantic::{AssetMetadata, IdentityId, ModuleSummary, TypeFact};
use stillwind_source::{
    CompileError, LintDirective, LintFinding, SourceFile, Span, collect_lint_directives,
};

use crate::Fingerprint;

/// Host-provided module identity, preserving the source path verbatim.
///
/// ```compile_fail
/// use stillwind_core::ModuleId;
/// let module: ModuleId = String::from("entry.tsx");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct ModuleId(pub String);

impl ModuleId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ModuleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::borrow::Borrow<str> for ModuleId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug)]
pub struct TypeProbe {
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ModuleScan {
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

#[derive(Clone, Debug)]
pub struct TypeFacts {
    pub schema: u32,
    pub module_id: ModuleId,
    pub source_revision: Fingerprint,
    pub semantic_revision: Fingerprint,
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
            asset_support: Vec::new(),
            external_assets: facts.map_or_else(BTreeSet::new, |facts| {
                let mut assets = BTreeSet::new();
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
    /// Attaches referenced declaration CSS without registering semantic inputs as producers.
    pub fn attach_asset_support(&mut self, inputs: &[Self]) {
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
                    identity.kind == stillwind_semantic::AssetKind::Animation
                        && identity.binding_name == declaration.name.value
                })
            });
            support
                .ir
                .view_transition_declarations
                .retain(|declaration| {
                    identities.iter().any(|identity| {
                        identity.kind == stillwind_semantic::AssetKind::ViewTransition
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
