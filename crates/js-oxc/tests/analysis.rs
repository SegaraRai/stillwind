use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::TypeFacts;
use stillwind_source::{FileId, Fingerprint, SourceFile};

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
