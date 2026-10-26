use lean_string::LeanStr;

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_source::{CssSourceName, FileId};
use stillwind_test_support::css_source;

use crate::{CandidateContext, CompilerOptions, CompletionInput, DesignSystemQuery};

#[test]
fn variant_completions_offer_valid_placeholders_and_skip_used_variants() {
    let source = css_source(FileId(1), "@import 'stillwind';").unwrap();
    let query = DesignSystemQuery::new(
        DesignSystemSnapshot::from_source(
            &source,
            "theme.css".into(),
            DesignSystemOptions::default(),
        )
        .unwrap(),
        CompilerOptions::default(),
    );
    let complete = |prefix: &str, context, snippets| {
        query
            .complete_candidate(CompletionInput {
                prefix: prefix.into(),
                context,
                snippets,
                markers: false,
            })
            .unwrap()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>()
    };
    let labels = complete("hover:", CandidateContext::Utility, true);
    assert!(!labels.iter().any(|label| label == "hover:hover:"));
    assert!(labels.iter().any(|label| label == "hover:nth-[]:"));
    assert!(labels.iter().any(|label| label == "hover:group-[]:"));
    assert!(
        complete("group-nth-", CandidateContext::Utility, true)
            .iter()
            .any(|label| label == "group-nth-[]:")
    );
    assert!(
        !complete("nth-", CandidateContext::Utility, false)
            .iter()
            .any(|label| label.contains("[]"))
    );
    assert_eq!(
        complete("nth-", CandidateContext::MediaVariants, true),
        [] as [LeanStr; 0]
    );
    assert!(
        complete("min-", CandidateContext::MediaVariants, true)
            .iter()
            .any(|label| label == "min-[]:")
    );
    assert!(
        query
            .preview_variant("hover:", CandidateContext::KeyframesFrame)
            .is_none()
    );
    assert!(
        query
            .preview_variant("hover:", CandidateContext::Document)
            .is_none()
    );
    assert!(
        query
            .preview_variant("hover:", CandidateContext::Utility)
            .unwrap()
            .contains("&:hover")
    );
}

#[test]
fn completions_include_theme_values_breakpoints_and_variant_prefixes() {
    let theme = css_source(
        FileId(1),
        "@import 'stillwind'; @theme { --color-brand: red; --breakpoint-tablet: 48rem; }",
    )
    .unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
    for (prefix, expected) in [
        ("bg-br", "bg-brand"),
        ("tab", "tablet:"),
        ("hov", "hover:"),
        ("hover:bg-br", "hover:bg-brand"),
        ("hover:tab", "hover:tablet:"),
        ("group-hov", "group-hover:"),
        ("group-not-fo", "group-not-focus:"),
        ("not-group-fo", "not-group-focus:"),
        ("hover:peer-fo", "hover:peer-focus:"),
        ("*", "*:"),
        ("**", "**:"),
    ] {
        let items = query
            .complete_candidate(CompletionInput {
                prefix: prefix.into(),
                context: CandidateContext::Utility,
                snippets: false,
                markers: false,
            })
            .unwrap();
        assert!(
            items.iter().any(|item| item.label == expected),
            "{prefix}: {items:?}"
        );
        assert!(
            items
                .iter()
                .all(|item| item.label.starts_with(prefix) && item.revision == snapshot.revision())
        );
        assert!(items.windows(2).all(|pair| pair[0].label < pair[1].label));
    }
    for (prefix, offered, omitted) in [
        ("not-s", "not-sm:", "not-starting:"),
        ("not-", "not-print:", "not-group-print:"),
        ("not-pl", "not-placeholder-shown:", "not-placeholder:"),
        ("group-", "group-required:", "group-sm:"),
        ("has-", "has-required:", "has-print:"),
    ] {
        let labels: Vec<_> = query
            .complete_candidate(CompletionInput {
                prefix: prefix.into(),
                context: CandidateContext::Utility,
                snippets: false,
                markers: false,
            })
            .unwrap()
            .into_iter()
            .map(|item| item.label)
            .collect();
        assert!(
            labels.iter().any(|label| label == offered),
            "{prefix}: {labels:?}"
        );
        assert!(
            labels.iter().all(|label| label != omitted),
            "{prefix}: {labels:?}"
        );
    }
    assert!(
        query
            .complete_candidate(CompletionInput {
                prefix: "missing:bg-br".into(),
                context: CandidateContext::Utility,
                snippets: false,
                markers: false,
            })
            .unwrap()
            .is_empty()
    );
}

/// In a document-level style, the first variant must select an element or be an at-rule
/// condition. Later variants can be any variant.
#[test]
fn document_completions_start_with_variants_that_select_an_element() {
    let theme = css_source(
        FileId(1),
        "@import 'stillwind'; @custom-variant shell scope(document) (:host);",
    )
    .unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    let labels = |prefix: &str| {
        query
            .complete_candidate(CompletionInput {
                prefix: prefix.into(),
                context: CandidateContext::Document,
                snippets: false,
                markers: false,
            })
            .unwrap()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>()
    };
    for (prefix, offered, omitted) in [
        ("h", Some("host:"), "hover:"),
        ("sh", Some("shell:"), "shell:flex"),
        ("pr", Some("print:"), "print:flex"),
        ("print:h", Some("print:host:"), "print:hover:"),
        ("host:h", Some("host:hover:"), "host:hidden:"),
        ("host:hid", Some("host:hidden"), "hidden"),
        ("not-h", None, "not-hover:"),
    ] {
        let labels = labels(prefix);
        assert!(
            offered.is_none_or(|offered| labels.iter().any(|label| label == offered)),
            "{prefix}: {labels:?}"
        );
        assert!(
            labels.iter().all(|label| label != omitted),
            "{prefix}: {labels:?}"
        );
    }
    assert_eq!(labels("hid"), [] as [LeanStr; 0]);
    assert_eq!(
        query
            .preview_candidate("host:hover:flex size-full", CandidateContext::Document)
            .unwrap()
            .as_deref(),
        Some("@media (hover: hover){:host:hover{display:flex;}}")
    );
    assert_eq!(
        query
            .preview_candidate("size-full", CandidateContext::Document)
            .unwrap(),
        None
    );
}

#[test]
fn utility_completions_offer_marker_names_only_where_they_add_the_class() {
    let source = css_source(
        FileId(1),
        "@stillwind-marker menu (modifiable);
            @utility card-title { .card & { color: red; } }
            @utility box { color: blue; }",
    )
    .unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    let items = |prefix: &str, markers: bool, snippets: bool| {
        query
            .complete_candidate(CompletionInput {
                prefix: prefix.into(),
                context: CandidateContext::Utility,
                snippets,
                markers,
            })
            .unwrap()
            .into_iter()
            .map(|item| (item.label.to_string(), item.marker))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        items("card", true, false),
        [("card".into(), true), ("card-title".into(), false)]
    );
    assert_eq!(
        items("menu", true, true),
        [("menu".into(), true), ("menu/".into(), true)]
    );
    assert_eq!(items("menu", true, false), [("menu".into(), true)]);
    assert_eq!(items("card", false, false), [("card-title".into(), false)]);
    for prefix in ["!card", "-card"] {
        assert!(
            items(prefix, true, false).iter().all(|(_, marker)| !marker),
            "{prefix}"
        );
    }
}
