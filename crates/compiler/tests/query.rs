//! Editor queries against a live snapshot.
//!
//! Pins that a query resolves the theme's own `@utility` definitions, that the declarations it
//! reports match what a real compilation emits for the same candidate, that every answer carries
//! the snapshot revision, and that hover, completion, and diagnostics agree about what resolves.

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
    // Queries use their own synthetic producer. Compare declarations, not identities.
    assert_eq!(resolved.generated_rules.len(), compiled.css.rules.len());
    for (queried, generated) in resolved.generated_rules.iter().zip(&compiled.css.rules) {
        assert_eq!(queried.prelude, generated.prelude);
        let declarations =
            |body: &Option<String>| body.as_ref().unwrap().split_once('{').unwrap().1.to_owned();
        assert_eq!(declarations(&queried.body), declarations(&generated.body));
    }
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
