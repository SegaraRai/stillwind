use super::*;

#[test]
fn resolves_directional_radius_theme_values() {
    let theme = theme();
    for candidate in [
        "rounded-s-sm",
        "rounded-ss-sm",
        "rounded-e-sm",
        "rounded-se-sm",
        "rounded-ee-sm",
        "rounded-es-sm",
        "rounded-t-sm",
        "rounded-l-sm",
        "rounded-tl-sm",
        "rounded-r-sm",
        "rounded-tr-sm",
        "rounded-b-sm",
        "rounded-br-sm",
        "rounded-bl-sm",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_some(),
            "{candidate} must resolve from --radius-sm"
        );
    }
}

#[test]
fn resolves_tailwind_static_keyword_utilities_from_the_css_registry() {
    let theme = theme();
    for (candidate, declaration) in [
        ("cursor-alias", "cursor:alias;"),
        ("overflow-scroll", "overflow:scroll;"),
        ("overscroll-y-none", "overscroll-behavior-y:none;"),
        ("break-after-page", "break-after:page;"),
        ("z-auto", "z-index:auto;"),
        ("z-73", "z-index:73;"),
        ("-z-73", "z-index:-73;"),
        ("border-13", "border-width:13px;"),
        ("col-span-full", "grid-column:1/-1;"),
        ("row-auto", "grid-row:auto;"),
        (
            "rounded-tl-full",
            "border-top-left-radius:calc(infinity * 1px);",
        ),
        ("touch-pan-left", "--tw-pan-x:pan-left;"),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            resolved.declaration.contains(declaration)
                || resolved
                    .additional
                    .iter()
                    .any(|target| target.declaration.contains(declaration)),
            "{candidate}: {resolved:#?}"
        );
    }
}

#[test]
fn resolves_discrete_tailwind_keywords_without_fixture_mappings() {
    let theme = theme();
    for (candidate, declaration, selector) in [
        (
            "divide-y",
            "border-bottom-style:var(--tw-border-style);border-top-style:var(--tw-border-style);border-top-width:calc(1px * var(--tw-divide-y-reverse));border-bottom-width:calc(1px * (1 - var(--tw-divide-y-reverse)));",
            Some(":where(&>:not(:last-child))"),
        ),
        (
            "divide-dashed",
            "border-style:dashed;",
            Some(":where(&>:not(:last-child))"),
        ),
        ("m-auto", "margin:auto;", None),
        ("mix-blend-color-dodge", "mix-blend-mode:color-dodge;", None),
        ("object-bottom-left", "object-position:left bottom;", None),
        (
            "outline",
            "outline-style:var(--tw-outline-style);outline-width:1px;",
            None,
        ),
        (
            "ring",
            "--tw-ring-shadow:var(--tw-ring-inset,) 0 0 0 calc(1px + var(--tw-ring-offset-width))",
            None,
        ),
        (
            "rounded-se-full",
            "border-start-end-radius:calc(infinity * 1px);",
            None,
        ),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            resolved.declaration.contains(declaration)
                || resolved
                    .additional
                    .iter()
                    .any(|target| target.declaration.contains(declaration)),
            "{candidate}: {resolved:#?}"
        );
        assert_eq!(
            resolved.selector.as_deref().or_else(|| resolved
                .additional
                .first()
                .and_then(|target| target.selector.as_deref())),
            selector,
            "{candidate}"
        );
    }
}

#[test]
fn resolves_fraction_and_theme_value_families_from_typed_css_domains() {
    let theme = theme();
    for (candidate, declaration) in [
        ("flex-5/12", "flex:calc(5 / 12 * 100%);"),
        ("columns-lg", "columns:var(--container-lg);"),
        ("font-stretch-125%", "font-stretch:125%;"),
        ("bg-radial", "--tw-gradient-position:in oklab;"),
        ("bg-conic", "--tw-gradient-position:in oklab;"),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            resolved.declaration.contains(declaration),
            "{candidate}: {resolved:#?}"
        );
    }
}

#[test]
fn resolves_text_lengths_and_additional_discrete_layout_keywords() {
    let theme = theme();
    for (candidate, declaration) in [
        ("text-[length:1.25rem]", "font-size:1.25rem;"),
        ("grid-cols-subgrid", "grid-template-columns:subgrid;"),
        ("leading-none", "line-height:1;"),
        ("origin-bottom-left", "transform-origin:bottom left;"),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            resolved.declaration.contains(declaration),
            "{candidate}: {resolved:#?}"
        );
    }

    let custom_theme = parse_tailwind_theme(
        &SourceFile::new(
            FileId(2),
            "theme.css",
            "@theme { --font-stretch-superbold: 150%; }",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        custom_theme
            .token_in(ThemeNamespace::FontStretch, "superbold")
            .is_some()
    );
    assert_eq!(
        resolve_utility("font-stretch-superbold", &custom_theme)
            .unwrap()
            .declaration,
        "font-stretch:var(--font-stretch-superbold);"
    );
}

#[test]
fn validates_unhinted_and_hint_checked_arbitrary_css_values() {
    let theme = theme();
    for candidate in [
        "p-[1rem]",
        "p-[50%]",
        "p-[calc(1rem_+_50%)]",
        "p-[length:1rem]",
        "p-[percentage:50%]",
        "border-[3px]",
        "border-[thin]",
        "z-[12]",
        "z-[auto]",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_some(),
            "expected `{candidate}` to resolve"
        );
    }
    assert_eq!(
        resolve_utility("p-[50%]", &theme).unwrap().declaration,
        "padding:50%;"
    );
    for candidate in [
        "p-[length:bogus]",
        "border-[time:1ms]",
        "border-[length:bogus]",
        "border-[50%]",
        "z-[1.5]",
        "z-[integer:1.5]",
        "z-[number:1]",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_none(),
            "expected `{candidate}` to be rejected"
        );
    }
}

#[test]
fn validates_arbitrary_utilities_against_property_grammars() {
    let theme = theme();
    for candidate in [
        "opacity-[.5]",
        "delay-[150ms]",
        "aspect-[16/9]",
        "grid-cols-[repeat(2,_minmax(0,_1fr))]",
        "content-['stillwind']",
        "mask-linear-[45deg,_red,_blue]",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_some(),
            "expected `{candidate}` to resolve"
        );
    }
    for candidate in [
        "opacity-[red]",
        "delay-[red]",
        "aspect-[red]",
        "grid-cols-[red]",
        "content-[red]",
        "mask-linear-[red_/_blue]",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_none(),
            "expected `{candidate}` to be rejected"
        );
    }
}

#[test]
fn matches_tailwind_unbracketed_value_grammar_catalog() {
    let catalog = include_str!("../../../../fixtures/tailwind-v4/catalog/value-grammar.md");
    let theme = theme();
    for (heading, expected) in [
        ("## Padding: accepted without brackets", true),
        ("## Padding: rejected without brackets", false),
        ("## Border width: accepted without brackets", true),
        ("## Border width: rejected without brackets", false),
        ("## Z index: accepted without brackets", true),
        ("## Z index: rejected without brackets", false),
    ] {
        for candidate in catalog_section(catalog, heading) {
            assert_eq!(
                resolve_utility(candidate, &theme).is_some(),
                expected,
                "{heading}: `{candidate}`"
            );
        }
    }
}
