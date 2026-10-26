//! Differential ordering against the pinned Tailwind compiler's unoptimized AST.

use stillwind_compiler::{Compiler, CompilerOptions};
use stillwind_design_system::{
    DesignSystemOptions, DesignSystemSnapshot, UtilityOrder, UtilityOrderMode, resolve_utility,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};
use stillwind_test_support::css_source;

#[test]
fn tailwind_resolved_property_sort_matches_upstream() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/tailwind-v4/utility-order.json"
    ))
    .unwrap();
    let source = css_source(FileId(1), fixture["stillwindCss"].as_str().unwrap()).unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("test.css"),
        DesignSystemOptions {
            utility_order: UtilityOrderMode::Tailwind,
            ..Default::default()
        },
    )
    .unwrap();
    let mut failures = Vec::new();
    let mut unresolved = Vec::new();
    let mut candidates = Vec::new();
    for expected in fixture["keys"].as_array().unwrap() {
        let candidate = expected["candidate"].as_str().unwrap();
        let Ok(resolved) = resolve_utility(candidate, snapshot.theme()).unwrap() else {
            unresolved.push(candidate);
            continue;
        };
        let UtilityOrder::Tailwind(actual) = &resolved.order else {
            panic!("ordering mode")
        };
        let order: Vec<usize> = serde_json::from_value(expected["order"].clone()).unwrap();
        let count = usize::try_from(expected["count"].as_u64().unwrap()).unwrap();
        if actual.properties != order || actual.count != count {
            failures.push(format!(
                "{candidate}: {actual:?}; expected {order:?}, {count}"
            ));
        }
        candidates.push((candidate, resolved.order));
    }
    assert_eq!(unresolved, ["max-w-prose", "shadow"]);
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
    candidates.reverse();
    candidates.sort_by(|(a, ka), (b, kb)| ka.compare(a, kb, b));
    let expected: Vec<_> = fixture["ordered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .filter(|candidate| !unresolved.contains(candidate))
        .collect();
    assert_eq!(
        candidates.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn emitted_precedence_uses_property_sequences_counts_and_numeric_names() {
    let theme = css_source(
        FileId(1),
        "
        @utility a-single { color:red; }
        @utility z-many { color:blue; --extra:; }
        @utility a-prefix { display:flex; }
        @utility z-long { display:block; color:green; }
        @utility item-2 { opacity:.2; }
        @utility item-10 { opacity:1; }
        @utility ordered-last (tw-sort: color) { z-index:7; }
        @utility ordered-first { display:grid; }
    ",
    )
    .unwrap();
    for utilities in [
        "a-single z-many a-prefix z-long item-10 item-2 ordered-last ordered-first",
        "ordered-first ordered-last item-2 item-10 z-long a-prefix z-many a-single",
    ] {
        let source = SourceFile::new(
            FileId(2),
            None,
            None,
            format!("import {{style}} from 'stillwind/react'; const value=style('{utilities}');"),
        )
        .unwrap();
        let output = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &source,
            &ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &theme,
            &CssSourceName::from("stillwind.css"),
            &CompilerOptions {
                utility_order: UtilityOrderMode::Tailwind,
                ..Default::default()
            },
        );
        assert!(output.diagnostics.is_empty(), "{output:#?}");
        let position = |text| output.css.find(text).unwrap();
        assert!(position("display:grid") < position("z-index:7"));
        assert!(position("display:block") < position("display:flex"));
        assert!(position("color:blue") < position("color:red"));
        assert!(position("opacity:.2") < position("opacity:1"));
        assert!(!output.css.contains("--tw-sort:"));
    }
}
