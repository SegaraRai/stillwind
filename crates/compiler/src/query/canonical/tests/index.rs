use super::*;

#[test]
fn functional_variants_can_shadow_bare_theme_names() {
    let query = make_query(
        "@theme { --breakpoint-lg:64rem; } \
         @custom-variant l* (&:focus); @custom-variant f (&:focus);",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("lg:flex").unwrap().as_deref(),
        Some("f:flex")
    );
}

#[test]
fn canonical_indexes_keep_numeric_overloads_for_numeric_theme_labels() {
    for (token, expected) in [("a", Some("span-a")), ("1", None)] {
        let query = make_query(
            &format!(
                "@theme {{ --*:initial; --width-{token}:1px; }} \
                 @utility span-* {{ width:--value(--width-*, [length]); }} \
                 @utility span-* {{ opacity:--value(number); }}"
            ),
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("span-[1px]").unwrap().as_deref(),
            expected,
            "{token}"
        );
    }
}

#[test]
fn canonical_comparisons_keep_spacing_transformations_separate() {
    let query = make_query(
        "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); } @utility p-0 { padding:2rem; } @utility p-1 { padding:calc(var(--spacing) * 4); }",
        CompilerOptions::default(),
    );
    for raw in ["p-[16px]", "p-[1rem]", "[padding:16px]"] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap(),
            Some("p-1".into()),
            "{raw}"
        );
    }
    assert_eq!(
        query.canonical_candidate("p-[15px]").unwrap(),
        Some("p-3.75".into())
    );
}

#[test]
fn canonical_search_preserves_nested_properties_and_overload_domains() {
    let query = make_query(
        "@theme { --color-brand:red; --spacing-x:1px; } @utility tone-* { &:hover { border-width:--value(--spacing-*, [length]); } } @utility tone-* { &:hover { border-color:--value(--color-*, [color]); } } @utility paint { &:hover { --paint:red; } }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("tone-[#f00]").unwrap(),
        Some("tone-brand".into())
    );
    assert_eq!(
        query.canonical_candidate("tone-[1px]").unwrap(),
        Some("tone-x".into())
    );
    assert!(
        query
            .canonical_candidate("[--paint:red]")
            .unwrap()
            .is_none()
    );
}

#[test]
fn canonical_variants_require_a_normalizable_value() {
    let query = make_query(
        "@theme { --color-loop:var(--color-loop); } @utility bg-* { background-color:--value(--color-*, [color]); } @variant over { &:hover { @slot; } }",
        CompilerOptions::default(),
    );
    assert!(
        query
            .canonical_candidate("[&:hover]:bg-loop")
            .unwrap()
            .is_none()
    );
}

#[test]
fn cloned_queries_can_reuse_previews_on_another_thread() {
    let query = make_query("@theme { --color-brand:red; }", CompilerOptions::default());
    assert_eq!(
        query.canonical_candidate("bg-[#f00]").unwrap(),
        Some("bg-brand".into())
    );
    let cloned = query.clone();
    let result =
        std::thread::spawn(move || cloned.canonical_candidate("bg-[rgb(255_0_0)]").unwrap())
            .join()
            .unwrap();
    assert_eq!(result, Some("bg-brand".into()));
    assert_eq!(
        query.canonical_candidate("bg-[rgb(255_0_0)]").unwrap(),
        result
    );
}

#[test]
fn canonical_templates_keep_static_aliases_quotes_and_requirements() {
    let query = make_query(
        "@theme { --*:initial; --color-a:red; --color-b:blue; --color-alias:var(--color-a); } @property --flag { syntax:'*'; inherits:false; initial-value:0; } @utility tone-* { @stillwind-require-properties --flag; &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); content:'var(--color-a)'; } }",
        CompilerOptions::default(),
    );
    for (raw, expected) in [("tone-[#f00]", "tone-a"), ("tone-[#00f]", "tone-b")] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap(),
            Some(expected.to_lean_str()),
            "{raw}"
        );
    }
}

#[test]
fn noncanonical_queries_defer_spelling_index_preparation() {
    let query = make_query(
        "@theme { --*:initial; --color-a:red; } @utility tone-* { color:--value(--color-*, [color]); }",
        CompilerOptions::default(),
    );
    assert_eq!(query.resolve_candidate("tone-a").unwrap().failures, []);
    assert!(query.canonical_candidate("tone-a").unwrap().is_none());
    assert!(
        query
            .canonical_candidate("tone-[length:1px]")
            .unwrap()
            .is_none()
    );
    assert!(query.canonical_index.get().is_none());
    assert_eq!(
        query.canonical_candidate("tone-[#f00]").unwrap(),
        Some("tone-a".into())
    );
    assert!(query.canonical_index.get().is_some());
}

#[test]
fn canonical_indexes_share_preparation_and_isolate_lint_policies() {
    let source = css_source(FileId(1), "@theme { --*:initial; --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); }").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("theme.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    prepare_canonical_index(&snapshot, &CompilerOptions::default());
    let prepared =
        QueryContext::new(snapshot.clone(), CompilerOptions::default()).canonical_index();
    let first = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
    assert!(Arc::ptr_eq(&prepared, first.canonical_index()));
    let second = DesignSystemQuery::new(
        snapshot.clone(),
        CompilerOptions {
            lint: LintOverrides {
                prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
                    severity: Some(LintSeverity::Deny),
                    ..Default::default()
                }),
                ..Default::default()
            },
            class_namespace: "another-project".into(),
            naming_revision: "another-generator".into(),
            hash_length: 12,
            ..Default::default()
        },
    );
    assert!(Arc::ptr_eq(
        first.canonical_index(),
        second.canonical_index()
    ));
    let disabled = DesignSystemQuery::new(
        snapshot,
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
    assert_eq!(
        first.canonical_candidate("p-[16px]").unwrap(),
        Some("p-4".into())
    );
    assert!(disabled.canonical_candidate("p-[16px]").unwrap().is_none());
}

#[test]
fn preparation_defers_requirement_errors_until_a_matching_candidate() {
    let source = css_source(FileId(1),
        "@utility good { color:red; } @utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        CssSourceName::from("theme.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    prepare_canonical_index(&snapshot, &CompilerOptions::default());
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    assert_eq!(query.resolve_candidate("good").unwrap().failures, []);
    assert_eq!(
        query.canonical_candidate("bad-[red]").unwrap_err().name,
        "bad-*"
    );
}

#[test]
fn canonical_modifiers_keep_branches_that_do_not_have_base_output() {
    let query = make_query(
        "@theme { --*:initial; --color-a:red; --color-b:blue; --text-body:1rem; --leading-tight:1.25rem; } @utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility flat-* { color:red; opacity:calc(--modifier(number) * 0 + 1); --x:calc(--value(--color-*, [color]) * 0); }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("text-[1rem]/tight").unwrap(),
        Some("text-body/tight".into())
    );
    assert_eq!(
        query.canonical_candidate("tone-[#f00]/50").unwrap(),
        Some("tone-a/50".into())
    );
    assert!(
        query
            .canonical_candidate("tone-[#123456]/50")
            .unwrap()
            .is_none()
    );
}

#[test]
fn canonical_indexes_keep_shadowing_families_and_exact_overrides() {
    let query = make_query(
        "@theme { --*:initial; --color-special-a:blue; --color-a:red; --color-b:blue; --color-empty:transparent; } @utility tone-* { color:--value(--color-*, [color]); } @utility tone-special-* { color:--value(--color-*, [color]); } @utility tone-a { color:blue; }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("tone-[#00f]").unwrap(),
        Some("tone-a".into())
    );
    assert_eq!(query.canonical_candidate("tone-[#f00]").unwrap(), None);
}

#[test]
fn canonical_modifiers_keep_overlapping_functional_branches() {
    let query = make_query(
        "@theme { --*:initial; --color-a:red; --color-special-a:blue; } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility tone-special-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility tone-a { color:blue; opacity:--modifier(number); }",
        CompilerOptions::default(),
    );
    assert_eq!(
        query.canonical_candidate("tone-[rgb(255_0_0)]/50").unwrap(),
        Some("tone-special-a/50".into())
    );
}

#[test]
fn canonical_color_modifiers_search_all_values_when_alpha_erases_the_color() {
    let query = make_query(
        "@theme { --*:initial; --color-a:red; --color-b:blue; } @theme inline { --opacity-50:0; } \
         @utility fixed-* { color:--compat-color(--value(--color-*, [color]), 0%); opacity:--modifier(number); } \
         @utility token-* { color:--compat-color(--value(--color-*, [color]), --alpha(--modifier(--opacity-*))); } \
         @utility extra-* { color:--compat-color(--value(--color-*, [color]), --alpha(--modifier()), 0%); }",
        CompilerOptions::default(),
    );
    for root in ["fixed", "token", "extra"] {
        let raw = format!("{root}-[#00f]/50");
        assert_eq!(
            query.canonical_candidate(&raw).unwrap().as_deref(),
            Some(format!("{root}-a/50").as_str()),
            "{root}"
        );
    }
}

#[test]
fn canonical_search_matches_finite_css_reference() {
    let query = make_query(
        "@theme { --*:initial; --spacing:.25rem; --spacing-a:1rem; --color-a:red; --color-b:blue; --color-c:rgb(255 0 0); --color-alias:var(--color-a); --text-body:1rem; --leading-tight:1.25rem; } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility plain-* { color:--value(--color-*, [color]); } @utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); } @utility nested-* { &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); } } @utility padded-* { padding:--value(--spacing-*, [length]); }",
        CompilerOptions::default(),
    );
    let index = query.canonical_index();
    for (root, value, modifier) in [
        ("tone", "#f00", "/50"),
        ("tone", "#123456", "/50"),
        ("plain", "rgb(255_0_0)", ""),
        ("plain", "#00f", ""),
        ("nested", "#00f", ""),
        ("padded", "1rem", ""),
        ("text", "1rem", "/tight"),
    ] {
        let raw = format_args!("{root}-[{value}]{modifier}").to_lean_str();
        let original = query.context.canonical_preview(&raw).unwrap().unwrap();
        let options = query.canonical_options();
        let scales = query.context.theme_scales();
        let mut labels: Vec<_> = index
            .labels
            .iter()
            .filter(|label| label.starts_with(format_args!("{root}-").to_lean_str().as_str()))
            .cloned()
            .collect();
        if value == "1rem" {
            labels.push(format_args!("{root}-4").to_lean_str());
        }
        labels.sort_by(|left, right| {
            (!index.labels.contains(left))
                .cmp(&(!index.labels.contains(right)))
                .then_with(|| rank(left, right))
        });
        let expected = labels.into_iter().find_map(|label| {
            let replacement = format_args!("{label}{modifier}").to_lean_str();
            if replacement.len() >= raw.len() {
                return None;
            }
            let preview = query.context.canonical_preview(&replacement).unwrap()?;
            let normalization = policy(&preview.css, &options, &scales);
            let signature = original.signature(
                &scales,
                &normalization.properties,
                query.context.options.root_font_size,
            );
            (signature.is_some()
                && preview.required_properties == original.required_properties
                && preview.signature(
                    &scales,
                    &normalization.properties,
                    query.context.options.root_font_size,
                ) == signature)
                .then_some(replacement)
        });
        assert_eq!(query.canonical_candidate(&raw).unwrap(), expected, "{raw}");
    }
}
