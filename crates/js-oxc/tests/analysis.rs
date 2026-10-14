//! Analyzer-level tests: fact validation and probe production.
//!
//! Both tests exercise the public [`SourceAnalyzer`] surface rather than the
//! extractor directly, because the contract a host depends on is which facts are
//! accepted and which spans are reported for a scan.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{
    AssetKind, CheckedValue, ExportedValue, IntegrationDiagnostic, SymbolicValue, TypeFact,
    TypeFacts,
};
use stillwind_source::{FileId, Fingerprint, ModuleId, SourceFile};

#[test]
fn checked_non_assets_and_wrong_families_are_distinct_from_unverified_values() {
    for (value, fatal) in [
        (Some(CheckedValue::NonAsset), false),
        (
            Some(
                ExportedValue::Keyframes(SymbolicValue::finite(AssetKind::Keyframes, []).unwrap())
                    .into(),
            ),
            false,
        ),
        (None, true),
        (
            Some(ExportedValue::Refs(SymbolicValue::universe(AssetKind::Ref)).into()),
            true,
        ),
        (
            Some(ExportedValue::Keyframes(SymbolicValue::universe(AssetKind::Keyframes)).into()),
            false,
        ),
    ] {
        let source = SourceFile::new(FileId(1), None, None,
            "import {style} from 'stillwind/react'; import {candidate} from './values'; const x=style('flex',candidate,candidate.asAnchor(),{[candidate('hover:')]: 'grid'});").unwrap();
        let facts = source
            .text()
            .match_indices("candidate")
            .skip(1)
            .filter_map(|(start, _)| {
                value.clone().map(|value| TypeFact {
                    start: u32::try_from(start).unwrap(),
                    end: u32::try_from(start + 9).unwrap(),
                    value,
                })
            })
            .collect::<Vec<_>>();
        let parsed = parse(
            &source,
            &ModuleId::from("consumer.ts"),
            &facts,
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        assert_eq!(
            !parsed.diagnostics.is_empty(),
            fatal,
            "{value:?}: {parsed:#?}"
        );
        if fatal {
            assert!(
                parsed.diagnostics.iter().all(|error| matches!(
                    error.diagnostic,
                    CompileDiagnostic::RefFactsRequired
                        | CompileDiagnostic::RefProjectionFactsRequired
                        | CompileDiagnostic::StyleInputFactsRequired
                )),
                "{parsed:#?}"
            );
        } else {
            assert_eq!(parsed.lint_findings.len(), 3, "{parsed:#?}");
            assert_eq!(parsed.module.style_calls[0].inputs.len(), 1, "{parsed:#?}");
        }
    }
}

#[test]
fn ref_variant_receivers_lower_each_conditional_branch() {
    for (receiver, inputs) in [
        ("factory()", 1),
        ("enabled ? ref : candidate", 1),
        ("enabled ? candidate : candidate", 0),
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!("import {{style}} from 'stillwind/react'; const ref=style.ref(); const x=style({{[({receiver})('hover:')]: 'flex'}});")).unwrap();
        let local = parse(
            &source,
            &ModuleId::from("consumer.ts"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        let definition = &local.module.ref_definitions[0];
        let identity = stillwind_semantic::IdentityId::new(
            AssetKind::Ref,
            definition.module_id.clone(),
            definition.binding.span.start.0,
            definition.binding.span.end.0,
            definition.binding.value.clone(),
        );
        let value = ExportedValue::Refs(SymbolicValue::exact(identity));
        let facts = source
            .text()
            .match_indices(if receiver == "factory()" {
                "factory()"
            } else {
                "candidate"
            })
            .map(|(start, text)| TypeFact {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + text.len()).unwrap(),
                value: if receiver == "factory()" {
                    value.clone().into()
                } else {
                    CheckedValue::NonAsset
                },
            })
            .collect::<Vec<_>>();
        let parsed = parse(
            &source,
            &ModuleId::from("consumer.ts"),
            &facts,
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        assert!(parsed.diagnostics.is_empty(), "{parsed:#?}");
        assert_eq!(
            parsed.module.style_calls[0].inputs.len(),
            inputs,
            "{receiver}: {parsed:#?}"
        );
        if receiver == "factory()" {
            assert!(parsed.lint_findings.is_empty(), "{parsed:#?}");
        } else {
            assert_eq!(
                parsed.lint_findings[0].diagnostic,
                LintDiagnostic::NonRefVariantReceiver
            );
        }
    }
}

#[test]
fn raw_ref_factories_work_as_scopes_when_the_checker_proves_their_identities() {
    for expression in ["factory()", "factory``"] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const ref=style.ref(); const x=style({{[ref('hover:').in({expression})]: 'flex'}}, ({expression}).asAnchor());"
        )).unwrap();
        let local = parse(
            &source,
            &ModuleId::from("consumer.ts"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        assert!(
            local.diagnostics.iter().any(|error| matches!(
                error.diagnostic,
                CompileDiagnostic::RefFactsRequired | CompileDiagnostic::RefProjectionFactsRequired
            )),
            "{local:#?}"
        );
        let definition = &local.module.ref_definitions[0];
        let identity = stillwind_semantic::IdentityId::new(
            AssetKind::Ref,
            definition.module_id.clone(),
            definition.binding.span.start.0,
            definition.binding.span.end.0,
            definition.binding.value.clone(),
        );
        let facts = source
            .text()
            .match_indices(expression)
            .map(|(start, text)| TypeFact {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + text.len()).unwrap(),
                value: ExportedValue::Refs(SymbolicValue::exact(identity.clone())).into(),
            })
            .collect::<Vec<_>>();
        let parsed = parse(
            &source,
            &ModuleId::from("consumer.ts"),
            &facts,
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Module,
        );
        assert!(parsed.diagnostics.is_empty(), "{parsed:#?}");
        assert!(parsed.lint_findings.is_empty(), "{parsed:#?}");
        assert_eq!(parsed.module.style_calls[0].inputs.len(), 2);
    }
}

#[test]
fn fresh_facts_with_invalid_symbolic_kinds_are_rejected_at_both_entries() {
    let source = Arc::new(SourceFile::new(FileId(1), None, None, "const x = 1;").unwrap());
    let scan = OxcSourceAnalyzer.scan(
        source.clone(),
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("fresh"),
        facts: vec![TypeFact {
            start: 10,
            end: 11,
            value: ExportedValue::Keyframes(SymbolicValue::universe(AssetKind::Ref)).into(),
        }],
        assets: BTreeMap::new(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    assert!(analysis.semantic_revision.is_none());
    assert_eq!(analysis.integration_diagnostics.len(), 1);
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &facts.facts,
        &facts.assets,
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.diagnostic == CompileDiagnostic::InvalidSymbolicFacts)
    );
}

/// Stale or malformed checker facts must be rejected without changing the IR.
///
/// The rejected run additionally reports why, and its `semantic_revision` stays `None`
/// so a cache cannot mistake it for a fact-backed analysis.
#[test]
fn analysis_is_design_system_independent_and_rejects_stale_facts() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; export const x = style('flex');",
        )
        .unwrap(),
    );
    let scan = OxcSourceAnalyzer.scan(
        source,
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(scan.local_ir.style_calls.len(), 1);
    let local = OxcSourceAnalyzer.finalize(scan.clone(), None);
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: Fingerprint::of("stale"),
        semantic_revision: Fingerprint::of("checker revision"),
        facts: vec![],
        assets: BTreeMap::default(),
    };
    let rejected = OxcSourceAnalyzer.finalize(scan.clone(), Some(&facts));
    assert_eq!(local.ir, rejected.ir);
    assert_eq!(rejected.integration_diagnostics.len(), 1);
    assert!(rejected.semantic_revision.is_none());
    let facts = TypeFacts {
        source_revision: scan.source_revision.clone(),
        ..facts
    };
    let accepted = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    assert_eq!(
        accepted.integration_diagnostics,
        [] as [IntegrationDiagnostic; 0]
    );
    assert_eq!(accepted.semantic_revision, Some(facts.semantic_revision));
    assert_eq!(accepted.ir, local.ir);
}

/// A value the frontend cannot resolve locally becomes a checker probe.
///
/// The probe span must slice the analysed source exactly, because the host matches its
/// own expressions by that span.
#[test]
fn scan_reports_expression_probes_for_non_local_asset_values() {
    let source = Arc::new(SourceFile::new(
        FileId(1), None, None,
        "import { style } from 'stillwind/react'; import { target } from './producer'; const x = style(target);",
    ).unwrap());
    let scan = OxcSourceAnalyzer.scan(
        source.clone(),
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(
        scan.type_probes
            .iter()
            .any(|probe| source.slice(probe.span).unwrap() == "target")
    );
}
