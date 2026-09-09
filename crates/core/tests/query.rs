use stillwind_core::{CompilerOptions, CompletionInput, DesignSystemQuery, DesignSystemSnapshot};
use stillwind_source::{FileId, SourceFile};

#[test]
fn queries_share_resolved_custom_definitions_and_pin_revisions() {
    let snapshot = DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(1), "theme", "@utility brand { color: red; }"),
        CompilerOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot.clone());
    let resolved = query.resolve_candidate("hover:brand");
    assert!(resolved.diagnostics.is_empty());
    assert_eq!(resolved.revision, snapshot.revision());
    let source = SourceFile::new(
        FileId(2),
        "a.tsx",
        "import { style } from 'stillwind/react'; const x = style('hover:brand');",
    );
    let analysis = stillwind_core::SourceAnalyzer::finalize(
        &stillwind_core::OxcSourceAnalyzer,
        stillwind_core::SourceAnalyzer::scan(
            &stillwind_core::OxcSourceAnalyzer,
            std::sync::Arc::new(source),
        ),
        None,
    );
    let compiled = stillwind_core::ModuleCompiler::new(snapshot.clone()).compile(&analysis);
    assert_eq!(resolved.generated_rules, compiled.css.rules);
    assert!(
        query
            .hover_candidate("brand")
            .unwrap()
            .css
            .replace(' ', "")
            .contains("color:red")
    );
    assert!(query.hover_candidate("missing:brand").is_none());
    assert!(!query.diagnose_candidate("this-does-not-exist").is_empty());
    assert!(
        query
            .complete_candidate(CompletionInput {
                prefix: "br".into()
            })
            .iter()
            .any(|item| item.label == "brand")
    );
}
