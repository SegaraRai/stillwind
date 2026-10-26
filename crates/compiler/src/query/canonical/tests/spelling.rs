use super::*;

#[test]
fn finite_aliases_compare_complete_declarations() {
    let query = make_query(
        "@import 'stillwind'; @theme { --example-a:8; } @utility example-* { --resolved-value: --value(--example, integer); }",
        CompilerOptions::default(),
    );
    for (raw, expected) in [
        ("example-8", "example-a"),
        ("-tracking-tight", "tracking-wide"),
        ("tracking-[0.025em]", "tracking-wide"),
        (
            "[color:var(--color-red-500)]/(--opacity)",
            "text-red-500/(--opacity)",
        ),
    ] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap().as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn runtime_conditions_keep_negation_parity_and_drop_empty_comparisons() {
    let query = make_query("@import 'stillwind';", CompilerOptions::default());
    for (raw, expected) in [
        ("if(!!--c):flex", Some("if(--c):flex")),
        ("if(!!!--c):flex", Some("if(!--c):flex")),
        ("if(--c:):flex", Some("if(--c):flex")),
        ("if(!--c):flex", None),
        (
            "[width:--if(!!--c,20px,10px)]",
            Some("w-[--if(--c,20px,10px)]"),
        ),
        (
            "w-[--if(!!!--c,20px,10px)]",
            Some("w-[--if(!--c,20px,10px)]"),
        ),
        (
            "[width:--if(--mode:,1px,2px)]",
            Some("w-[--if(--mode,1px,2px)]"),
        ),
        (
            "[width:--if(--c,20px,10px)]",
            Some("w-[--if(--c,20px,10px)]"),
        ),
    ] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap().as_deref(),
            expected,
            "{raw}"
        );
    }
}

#[test]
fn dynamic_families_with_both_forms_prefer_the_named_spelling() {
    let pending = make_query(
        "@stillwind-dynamic icon-*; @stillwind-dynamic icon-[*];",
        CompilerOptions::default(),
    );
    assert!(
        pending
            .canonical_candidate("icon-[mdi--close]")
            .unwrap()
            .is_none()
    );
    let query = make_query(
        "@import 'stillwind'; @stillwind-dynamic icon-*; @stillwind-dynamic icon-[*]; @stillwind-dynamic i-[*]; @utility icon-* { --size: --value([length]); } @utility icon-mdi--close { display: inline-block; }",
        CompilerOptions::default(),
    );
    for (raw, expected) in [
        ("icon-[mdi--close]", Some("icon-mdi--close")),
        ("icon-[2rem]", None),
        ("icon-[red]", None),
        ("icon-[mdi--missing]", None),
        ("hover:!icon-[mdi--close]", Some("hover:icon-mdi--close!")),
        ("unknown:icon-[mdi--close]", None),
        ("icon-[mdi--close]/50", None),
        ("-icon-[mdi--close]", None),
        ("icon-mdi--close", None),
        ("i-[mdi--close]", None),
    ] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap().as_deref(),
            expected,
            "{raw}"
        );
    }
}

#[test]
fn canonical_spacing_respects_units_and_preserves_variants_modifiers_and_importance() {
    let query = make_query(
        "@import 'stillwind'; @theme { --spacing: .25rem; }",
        CompilerOptions::default(),
    );
    for (raw, expected) in [
        ("p-[16px]", "p-4"),
        ("p-[1rem]", "p-4"),
        ("border-[2px]", "border-2"),
        ("hover:!p-[16px]", "hover:p-4!"),
        ("[padding:16px]", "p-4"),
    ] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap(),
            Some(expected.to_lean_str()),
            "{raw}"
        );
        assert!(query.canonical_candidate(expected).unwrap().is_none());
    }
    let query = make_query(
        "@import 'stillwind'; @theme { --spacing: .25rem; }",
        CompilerOptions {
            lint: LintOverrides {
                prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
                    px_to_rem: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(query.canonical_candidate("p-[16px]").unwrap().is_none());
    assert_eq!(
        query.canonical_candidate("border-[2px]").unwrap(),
        Some("border-2".into())
    );
}

#[test]
fn canonical_queries_use_project_font_size_and_the_actual_custom_definition() {
    let query = make_query(
        "@import 'stillwind'; @theme { --spacing: .25rem; }",
        CompilerOptions {
            root_font_size: RootFontSize::new(20.5).unwrap(),
            ..Default::default()
        },
    );
    assert_eq!(
        query.canonical_candidate("p-[20.5px]").unwrap(),
        Some("p-4".into())
    );
    let query = make_query(
        "@import 'stillwind'; @theme { --spacing: .25rem; } @utility p-4 { padding: 2rem; }",
        CompilerOptions::default(),
    );
    assert!(query.canonical_candidate("p-[16px]").unwrap().is_none());
}

#[test]
fn spacing_conversion_preserves_selectors_strings_and_nonspacing_declarations() {
    let query = make_query(
        "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); content:'16px'; border-width:2px; } @utility p-4 { padding:calc(var(--spacing) * 4); content:'1rem'; border-width:2px; }",
        CompilerOptions::default(),
    );
    assert!(query.canonical_candidate("p-[16px]").unwrap().is_none());
    assert_eq!(
        convert_pixel_dimensions("calc(16px + 2px) '16px' url(16px)", 16.0),
        "calc(1rem + 0.125rem) '16px' url(16px)"
    );
}

#[test]
fn canonical_replacements_resolve_named_spacing_and_color_tokens() {
    let query = make_query(
        "@theme { --spacing:.25rem; --spacing-4:1rem; --color-brand:red; } @utility p-* { padding:--value(--spacing-*, --spacing(), [length]); } @utility bg-* { background-color:--value(--color-*, [color]); }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("p-[16px]").unwrap(),
        Some("p-4".into())
    );
    assert_eq!(
        query.canonical_candidate("bg-[#f00]").unwrap(),
        Some("bg-brand".into())
    );
}

#[test]
fn canonical_color_modifiers_match_with_the_default_registry() {
    let query = make_query(
        "@import 'stillwind'; @theme { --color-brand:red; --color-b:blue; }",
        CompilerOptions::default(),
    );
    for (raw, expected) in [
        ("bg-[#f00]/50", "bg-brand/50"),
        ("hover:!bg-[#f00]/50", "hover:bg-brand/50!"),
        ("bg-[#f00]/100", "bg-brand"),
        ("bg-[#f00]/0", "bg-b/0"),
    ] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap().as_deref(),
            Some(expected),
            "{raw}"
        );
    }
    assert!(
        query
            .canonical_candidate("bg-[#123456]/50")
            .unwrap()
            .is_none()
    );
    assert!(
        query
            .hover_candidate("bg-brand/50")
            .unwrap()
            .unwrap()
            .css
            .contains("@supports")
    );
}

#[test]
fn canonical_color_compatibility_keeps_authored_conditions() {
    let query = make_query(
        "@theme { --*:initial; --color-brand:red; } \
         @utility tone-* { @media print { color:--compat-color(--value(--color-*, [color]), --alpha(--modifier())); } } \
         @utility paint-* { color:--value(--color-*, [color]); } \
         @utility paint-brand { color:red; @supports (color:color-mix(in lab, red, red)) { color:blue; } }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("tone-[#f00]/50").unwrap(),
        Some("tone-brand/50".into())
    );
    assert!(query.canonical_candidate("paint-[#f00]").unwrap().is_none());
}

#[test]
fn canonical_overloads_search_tokens_for_the_emitted_properties() {
    let query = make_query(
        "@import 'stillwind'; @theme { --spacing:.25rem; }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query
            .canonical_candidate("border-[line-width:thin]")
            .unwrap()
            .as_deref(),
        Some("border-[thin]")
    );
    assert_eq!(
        query
            .canonical_candidate("p-[length-percentage:calc(3px_+_50%)]")
            .unwrap()
            .as_deref(),
        Some("p-[calc(3px+50%)]")
    );
    let query = make_query(
        "@theme { --color-brand:red; --spacing-x:1px; } @utility tone-* { border-width:--value(--spacing-*, [length]); } @utility tone-* { border-color:--value(--color-*, [color]); }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("tone-[1px]").unwrap(),
        Some("tone-x".into())
    );
    assert_eq!(
        query.canonical_candidate("tone-[#f00]").unwrap(),
        Some("tone-brand".into())
    );
}

#[test]
fn canonical_filter_keeps_arbitrary_utilities_and_variants() {
    let query = make_query(
        "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); } @variant over { &:hover { @slot; } }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("[&:hover]:p-4").unwrap(),
        Some("over:p-4".into())
    );
    assert_eq!(
        query.canonical_candidate("over:!p-[16px]").unwrap(),
        Some("over:p-4!".into())
    );
    assert_eq!(
        query
            .canonical_candidate("p-(--spacing)")
            .unwrap()
            .as_deref(),
        Some("p-1")
    );
    for raw in ["p-4", "over:p-4", "p-4/[2]", "invalid["] {
        assert!(query.canonical_candidate(raw).unwrap().is_none(), "{raw}");
    }
}
