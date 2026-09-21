//! Editor queries against a live snapshot.
//!
//! Pins that a query resolves the theme's own `@utility` definitions, that the declarations it
//! reports match what a real compilation emits for the same candidate, that every answer carries
//! the snapshot revision, and that hover, completion, and diagnostics agree about what resolves.

use std::sync::Arc;

use stillwind_compiler::{
    Compiler, CompilerOptions, CompletionInput, DesignSystemQuery, ModuleCompiler,
};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{LintKind, UtilityFailure};
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile};

#[test]
fn registration_order_mixes_user_and_builtin_definitions_without_moving_overrides() {
    let css = "@variant before { &:first-child { @slot; } }
        @stillwind-variant middle { &:hover { @slot; } }
        @variant after { &:last-child { @slot; } }
        @variant middle { &:focus { @slot; } }
        @utility early { color:red; }
        @stillwind-utility builtin { color:blue; }
        @utility late { color:green; }
        @utility early { color:purple; }";
    let mut ordering_fingerprints = Vec::new();
    for (mode, expected) in [
        ("authored", ["early", "builtin", "late"]),
        ("tailwind", ["builtin", "early", "late"]),
        ("", ["early", "builtin", "late"]),
    ] {
        let config = if mode.is_empty() {
            String::new()
        } else {
            format!("@stillwind {{utility-order:{mode};}}")
        };
        let source = SourceFile::new(
            FileId(1),
            "theme.css",
            format!("{css} {config} @stillwind-slot utilities;"),
        )
        .unwrap();
        let snapshot =
            DesignSystemSnapshot::from_source(&source, DesignSystemOptions::default()).unwrap();
        ordering_fingerprints.push(snapshot.fingerprints().ordering.clone());
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        let mut utilities = ["late", "builtin", "early"];
        utilities.sort_by(|a, b| query.compare_candidates(a, b).unwrap());
        assert_eq!(utilities, expected);
        let mut variants = ["after:early", "middle:early", "before:early"];
        variants.sort_by(|a, b| query.compare_candidates(a, b).unwrap());
        assert_eq!(variants, ["before:early", "middle:early", "after:early"]);
        assert!(
            query
                .hover_candidate("middle:early")
                .unwrap()
                .css
                .contains(":focus")
        );
        let input = SourceFile::new(
            FileId(2),
            "entry.ts",
            "import {style} from 'stillwind/react'; const value=style('late builtin early');",
        )
        .unwrap();
        let compiled = Compiler::compile_source(&OxcSourceAnalyzer, &input, &source);
        assert!(compiled.diagnostics.is_empty(), "{compiled:#?}");
        let blue = compiled.css.find("color:blue").unwrap();
        let purple = compiled.css.find("color:purple").unwrap();
        assert_eq!(purple < blue, mode != "tailwind", "{}", compiled.css);
    }
    assert_ne!(ordering_fingerprints[0], ordering_fingerprints[1]);
    assert_eq!(ordering_fingerprints[0], ordering_fingerprints[2]);
}

#[test]
fn editor_order_uses_custom_utility_and_variant_precedence() {
    let theme = crate::css_fixture::source(
        FileId(1),
        "theme",
        "@utility a { color:red; } @utility b { color:blue; }",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    let mut candidates = ["hover:b", "b", "hover:a", "a", "flex"];
    candidates.sort_by(|left, right| query.compare_candidates(left, right).unwrap());
    assert_eq!(candidates, ["flex", "a", "b", "hover:a", "hover:b"]);
    assert_eq!(
        query.compare_candidates("a", "a"),
        Some(std::cmp::Ordering::Equal)
    );
    assert_eq!(query.compare_candidates("unknown:a", "a"), None);
    assert_eq!(query.compare_candidates("a", "unknown-utility"), None);
}

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
    for (candidate, classification) in [
        ("hover:[", UtilityFailure::Invalid),
        ("不存在", UtilityFailure::Unknown),
    ] {
        let findings = query.diagnose_candidate(candidate);
        assert_eq!(findings, query.resolve_candidate(candidate).diagnostics);
        assert_eq!(
            findings[0].kind,
            LintKind::UnresolvedCandidate {
                candidate: candidate.into(),
                classification
            }
        );
        assert_eq!(
            findings[0].to_string(),
            format!("unresolved candidate: {candidate}")
        );
        assert_eq!(findings[0].labels[0].span.file, FileId(0));
        assert_eq!(findings[0].labels[0].span.start.0, 0);
        assert_eq!(findings[0].labels[0].span.end.0 as usize, candidate.len());
    }
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
