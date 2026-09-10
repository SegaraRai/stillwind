use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, CompletionInput, DesignSystemQuery, ModuleCompiler};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile};

#[test]
fn queries_share_resolved_custom_definitions_and_pin_revisions() {
    let theme =
        crate::css_fixture::source(FileId(1), "theme", "@utility brand { color: red; }").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
    let query = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
    let resolved = query.resolve_candidate("hover:brand");
    assert!(resolved.diagnostics.is_empty());
    assert_eq!(resolved.revision, snapshot.revision());
    let source = SourceFile::new(
        FileId(2),
        "a.tsx",
        "import { style } from 'stillwind/react'; const x = style('hover:brand');",
    )
    .unwrap();
    let analysis = SourceAnalyzer::finalize(
        &OxcSourceAnalyzer,
        SourceAnalyzer::scan(&OxcSourceAnalyzer, Arc::new(source)),
        None,
    );
    let compiled =
        ModuleCompiler::new(snapshot.clone(), CompilerOptions::default()).compile(&analysis);
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

#[path = "../../test-support/css.rs"]
mod css_fixture;
