use std::sync::Arc;
use stillwind_core::{Fingerprint, OxcSourceAnalyzer, SourceAnalyzer, TypeFacts};
use stillwind_source::{FileId, SourceFile};

#[test]
fn analysis_is_design_system_independent_and_rejects_stale_facts() {
    let source = Arc::new(SourceFile::new(
        FileId(1),
        "Button.tsx",
        "import { style } from 'stillwind/react'; export const x = style('flex');",
    ));
    let scan = OxcSourceAnalyzer.scan(source);
    assert_eq!(scan.local_ir.style_calls.len(), 1);
    let local = OxcSourceAnalyzer.finalize(scan.clone(), None);
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: Fingerprint::of("stale"),
        semantic_revision: Fingerprint::of("checker revision"),
        facts: vec![],
        assets: std::collections::BTreeMap::default(),
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
