use lean_string::LeanStr;

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_source::{CssSourceName, FileId, SourceFile};
use stillwind_test_support::css_source;

use super::editor_css_source;
use crate::CompilerOptions;
use crate::query::DesignSystemQuery;

fn query() -> DesignSystemQuery {
    let source = SourceFile::new(FileId(1), None, None, "@theme { --color-brand: red; --color-muted: blue; } @utility flex { display:flex; } @utility bg-* { background-color:--value(--color-*); } @custom-variant hover (&:hover); @custom-variant dark { @media (prefers-color-scheme:dark) { @slot; } }").unwrap();
    DesignSystemQuery::new(
        DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap(),
        CompilerOptions::default(),
    )
}

fn labels(css: &str) -> Vec<LeanStr> {
    query()
        .complete_stylesheet(css, css.len())
        .unwrap()
        .into_iter()
        .map(|item| item.label)
        .collect()
}

#[test]
fn completes_partial_directives_applications_variants_and_slots() {
    assert!(labels("@").contains(&"@apply".into()));
    assert!(labels("/* note */ @").contains(&"@apply".into()));
    assert!(labels(".a { color:red; /* note */ @").contains(&"@apply".into()));
    assert_eq!(labels("@APPl"), ["@apply"]);
    assert!(labels(".a { @app").contains(&"@apply".into()));
    assert_eq!(labels(".a { @apply fl"), ["flex"]);
    assert_eq!(labels("/* 日本語 🦀 */ .a { @apply fl"), ["flex"]);
    assert!(labels(".a { @apply ").contains(&"flex".into()));
    assert!(labels(".a { @variant ").contains(&"hover".into()));
    assert_eq!(labels(".a { @apply hover:bg-br"), ["hover:bg-brand"]);
    assert_eq!(labels(".a { @variant ho"), ["hover"]);
    assert_eq!(labels(".a { @variant hover "), ["hover"]);
    assert_eq!(labels(".a { @variant dark:ho"), ["dark:hover"]);
    assert_eq!(labels(".a { @variant dark, ho"), ["hover"]);
    assert_eq!(labels("@variant ho"), [] as [LeanStr; 0]);
    assert_eq!(labels("@stillwind-slot util"), ["utilities"]);
    assert_eq!(
        labels(".a { @stillwind-slot property-initials inh"),
        ["property-initials inherited"]
    );
    assert_eq!(
        labels(".a { @stillwind-slot property-initials\tinh"),
        ["property-initials inherited"]
    );
    assert!(!labels(".a { @stillwind-slot ").contains(&"utilities".into()));
}

#[test]
fn theme_references_replace_the_whole_name_and_skip_fallback_text() {
    let css = "/* 日本語 🦀 */ .a { color: --theme(--color-brZZ); }";
    let offset = css.find("brZZ").unwrap() + 2;
    let items = query().complete_stylesheet(css, offset).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "--color-brand");
    assert_eq!(&css[items[0].range.clone()], "--color-brZZ");
    for name in ["theme", "--theme", "var"] {
        assert!(labels(&format!(".a {{ color:{name}( ")).contains(&"--color-brand".into()));
        assert!(
            labels(&format!(".a {{ color:{name}(--color-br")).contains(&"--color-brand".into())
        );
        assert_eq!(
            labels(&format!(".a {{ color:{name}(--color-brand, ")),
            [] as [LeanStr; 0]
        );
    }
}

#[test]
fn strings_comments_urls_and_definition_headers_do_not_offer_semantic_labels() {
    for css in [
        "/* @apply fl",
        ".a { content:'@apply fl",
        ".a { background:url(@apply fl",
        ".a { @apply /* fl",
        "@custom-variant ho",
        "@utility fl",
    ] {
        assert!(labels(css).is_empty(), "{css}");
    }
    assert!(query().complete_stylesheet("🦀", 1).unwrap().is_empty());
    assert!(query().complete_stylesheet("", 1).unwrap().is_empty());
}

#[test]
fn hovers_share_utility_and_variant_expansion_and_describe_dialect_inputs() {
    let query = query();
    for (css, expected) in [
        (".a { @apply hover:flex", "display:flex"),
        (".a { @variant dark:hover", "&:hover"),
        (".a { @variant hover", "&:hover"),
    ] {
        assert!(
            query
                .hover_stylesheet(css, css.len())
                .unwrap()
                .unwrap()
                .css
                .contains(expected)
        );
    }
    for (css, expected) in [
        ("@apply", "@apply utilities;"),
        ("@stillwind-slot properties", "@property"),
        (
            ".a { @stillwind-slot property-initials inherited",
            "inherits: true",
        ),
    ] {
        assert!(
            query
                .hover_stylesheet(css, css.len())
                .unwrap()
                .unwrap()
                .documentation
                .contains(expected)
        );
    }
    let css = ".a { color:--theme(--color-brand";
    assert_eq!(
        query.hover_stylesheet(css, css.len()).unwrap().unwrap().css,
        "--color-brand: red;"
    );
}

#[test]
fn recovery_retains_imports_and_definitions_from_unclosed_sources() {
    let css = "@import './tokens.css'; @utility flex {display:flex;} @custom-variant hover { &:hover { @slot; } } .a { @apply fl";
    let recovered = editor_css_source(css);
    assert!(recovered.contains("@import './tokens.css';"));
    assert!(recovered.contains("@utility flex{"));
    assert!(recovered.contains("@slot;"));
    let source = css_source(
        FileId(1),
        editor_css_source("@theme { --color-brand:red; --color-next: --theme(--col"),
    )
    .unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        "theme.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    assert_eq!(
        snapshot
            .theme()
            .custom_property("--color-brand")
            .unwrap()
            .value
            .value,
        "red"
    );
}
