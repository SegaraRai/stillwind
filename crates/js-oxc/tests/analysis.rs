//! Analyzer-level tests: fact validation and probe production.
//!
//! Both tests exercise the public [`SourceAnalyzer`] surface rather than the
//! extractor directly, because the contract a host depends on is which facts are
//! accepted and which spans are reported for a scan.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::{OxcSourceAnalyzer, parse_with_type_facts};
use stillwind_semantic::{AssetKind, ExportedValue, SymbolicValue, TypeFact, TypeFacts};
use stillwind_source::{FileId, Fingerprint, SourceFile};

#[test]
fn fresh_facts_with_invalid_symbolic_kinds_are_rejected_at_both_entries() {
    let source = Arc::new(SourceFile::new(FileId(1), "entry.tsx", "const x = 1;").unwrap());
    let scan = OxcSourceAnalyzer.scan(source.clone());
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("fresh"),
        facts: vec![TypeFact {
            start: 10,
            end: 11,
            value: ExportedValue::Keyframes(SymbolicValue::universe(AssetKind::Subtree)),
        }],
        assets: BTreeMap::new(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    assert!(analysis.semantic_revision.is_none());
    assert_eq!(analysis.integration_diagnostics.len(), 1);
    let parsed = parse_with_type_facts(&source, &facts.facts, &facts.assets);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.kind == CompileErrorKind::InvalidSymbolicFacts)
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
            "Button.tsx",
            "import { style } from 'stillwind/react'; export const x = style('flex');",
        )
        .unwrap(),
    );
    let scan = OxcSourceAnalyzer.scan(source);
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
    assert!(accepted.integration_diagnostics.is_empty());
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
        FileId(1),
        "consumer.tsx",
        "import { style } from 'stillwind/react'; import { target } from './producer'; const x = style(target);",
    ).unwrap());
    let scan = OxcSourceAnalyzer.scan(source.clone());
    assert!(
        scan.type_probes
            .iter()
            .any(|probe| source.slice(probe.span).unwrap() == "target")
    );
}
