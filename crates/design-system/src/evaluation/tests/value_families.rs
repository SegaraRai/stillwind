//! Value-family coverage: theme values, keywords, fractions, and arbitrary values.
//!
//! Pins discrete keyword utilities, directional theme values, fraction and theme
//! families, and the difference between unhinted and hint-checked arbitrary values
//! against Tailwind's unbracketed grammar catalog.

use stillwind_test_support::{CssAssertions as _, assert_css_eq, catalog_section};

use super::*;
use crate::test_support::parse_fixture_theme;

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
            resolved.declaration.contains_css(declaration)
                || resolved
                    .additional
                    .iter()
                    .any(|target| target.declaration.contains_css(declaration)),
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
            Some(":where(& > :not(:last-child))"),
        ),
        (
            "divide-dashed",
            "border-style:dashed;",
            Some(":where(& > :not(:last-child))"),
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
            resolved.declaration.contains_css(declaration)
                || resolved
                    .additional
                    .iter()
                    .any(|target| target.declaration.contains_css(declaration)),
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
            resolved.declaration.contains_css(declaration),
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
            resolved.declaration.contains_css(declaration),
            "{candidate}: {resolved:#?}"
        );
    }

    let custom_theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(2),
            None,
            None,
            "@theme { --font-stretch-superbold: 150%; }",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(custom_theme.token_in("font-stretch", "superbold").is_some());
    assert_css_eq!(
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
    assert_css_eq!(
        resolve_utility("p-[50%]", &theme).unwrap().declaration,
        "padding:50%;"
    );
    for candidate in [
        "p-[length:bogus]",
        "border-[time:1ms]",
        "border-[length:bogus]",
        "z-[integer:1.5]",
        "z-[number:1]",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_none(),
            "expected `{candidate}` to be rejected"
        );
    }
    for candidate in ["border-[50%]", "z-[1.5]"] {
        assert!(
            resolve_utility(candidate, &theme)
                .is_some_and(|resolved| resolved.diagnostic_data.invalid_value.is_some()),
            "expected `{candidate}` to be emitted as an invalid property value"
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
            resolve_utility(candidate, &theme)
                .is_some_and(|resolved| resolved.diagnostic_data.invalid_value.is_some()),
            "expected `{candidate}` to be emitted as an invalid property value"
        );
    }
}

#[test]
fn matches_tailwind_unbracketed_value_grammar_catalog() {
    let catalog = include_str!("../../../../../fixtures/tailwind-v4/catalog/value-grammar.md");
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

/// A theme that defines one token in each namespace that a Tailwind utility reads.
fn theme_with_utility_namespaces() -> ThemeIndex {
    parse_fixture_theme(
        &SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme {
              --order-x: 3; --grid-column-x: 1 / 3; --grid-column-start-x: 2;
              --grid-column-end-x: 3; --grid-row-x: 1 / 3; --grid-row-start-x: 2;
              --grid-row-end-x: 3; --grid-template-columns-x: repeat(2, 1fr);
              --grid-template-rows-x: repeat(2, 1fr); --grid-auto-columns-x: 10rem;
              --grid-auto-rows-x: 10rem; --scale-big: 150%; --rotate-big: 45deg;
              --skew-big: 6deg; --transform-origin-corner: top left;
              --perspective-deep: 900px; --perspective-origin-corner: top left;
              --brightness-big: 150%; --contrast-big: 150%; --backdrop-brightness-big: 120%;
              --hue-rotate-big: 30deg; --opacity-half: 50%;
              --transition-property-fancy: opacity; --transition-delay-slow: 300ms;
              --transition-duration-slow: 300ms; --cursor-fancy: pointer;
              --content-mark: '*'; --line-clamp-few: 3; --list-style-type-fancy: square;
              --list-style-image-dot: url(dot.png); --object-position-corner: top left;
              --divide-width-thick: 3px; --border-width-thin: 2px;
            }",
        )
        .unwrap(),
    )
    .unwrap()
}

/// Returns the declarations of `resolved`, including those of its nested rules.
fn all_declarations(resolved: &ResolvedUtility) -> String {
    std::iter::once(resolved.declaration.as_str())
        .chain(resolved.nested.iter().map(|rule| rule.declaration.as_str()))
        .collect::<String>()
        .replace(' ', "")
}

#[test]
fn functional_utilities_read_the_theme_namespaces_that_tailwind_reads() {
    let theme = theme_with_utility_namespaces();
    for (candidate, token) in [
        ("order-x", "--order-x"),
        ("-order-x", "--order-x"),
        ("col-x", "--grid-column-x"),
        ("-col-start-x", "--grid-column-start-x"),
        ("col-end-x", "--grid-column-end-x"),
        ("row-x", "--grid-row-x"),
        ("row-start-x", "--grid-row-start-x"),
        ("-row-end-x", "--grid-row-end-x"),
        ("grid-cols-x", "--grid-template-columns-x"),
        ("grid-rows-x", "--grid-template-rows-x"),
        ("auto-cols-x", "--grid-auto-columns-x"),
        ("auto-rows-x", "--grid-auto-rows-x"),
        ("scale-big", "--scale-big"),
        ("-scale-x-big", "--scale-big"),
        ("scale-z-big", "--scale-big"),
        ("rotate-big", "--rotate-big"),
        ("-rotate-y-big", "--rotate-big"),
        ("skew-big", "--skew-big"),
        ("-skew-x-big", "--skew-big"),
        ("origin-corner", "--transform-origin-corner"),
        ("perspective-deep", "--perspective-deep"),
        ("perspective-origin-corner", "--perspective-origin-corner"),
        ("brightness-big", "--brightness-big"),
        ("backdrop-brightness-big", "--backdrop-brightness-big"),
        ("backdrop-contrast-big", "--contrast-big"),
        ("hue-rotate-big", "--hue-rotate-big"),
        ("-hue-rotate-big", "--hue-rotate-big"),
        ("-backdrop-hue-rotate-big", "--hue-rotate-big"),
        ("opacity-half", "--opacity-half"),
        ("backdrop-opacity-half", "--opacity-half"),
        ("transition-fancy", "--transition-property-fancy"),
        ("delay-slow", "--transition-delay-slow"),
        ("duration-slow", "--transition-duration-slow"),
        ("cursor-fancy", "--cursor-fancy"),
        ("content-mark", "--content-mark"),
        ("line-clamp-few", "--line-clamp-few"),
        ("list-fancy", "--list-style-type-fancy"),
        ("list-image-dot", "--list-style-image-dot"),
        ("object-corner", "--object-position-corner"),
        ("divide-x-thick", "--divide-width-thick"),
        ("divide-y-thin", "--border-width-thin"),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            all_declarations(&resolved).contains(&format!("var({token})")),
            "`{candidate}` must read {token}: {}",
            all_declarations(&resolved)
        );
    }
}

#[test]
fn accepts_tailwind_bare_and_negative_values() {
    let theme = theme();
    for (candidate, declaration) in [
        ("-col-3", "grid-column:calc(3*-1);"),
        ("-row-2", "grid-row:calc(2*-1);"),
        ("auto-cols-3", "grid-auto-columns:calc(var(--spacing)*3);"),
        ("opacity-1.5", "opacity:calc(1.5*1%);"),
        ("flex-grow-2", "flex-grow:2;"),
        ("flex-shrink-0", "flex-shrink:0;"),
        ("max-w-prose", "max-width:65ch;"),
        ("w-auto", "width:auto;"),
        ("min-h-auto", "min-height:auto;"),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            all_declarations(&resolved).contains(declaration),
            "`{candidate}` must declare {declaration}: {}",
            all_declarations(&resolved)
        );
    }
    let resolved = resolve_utility("-hue-rotate-30", &theme).unwrap();
    assert!(all_declarations(&resolved).contains("hue-rotate(calc(30*1deg*-1))"));
}

#[test]
fn rejects_values_that_produce_invalid_css() {
    let theme = theme();
    for candidate in [
        // `<grid-line>` and `span` do not accept 0.
        "col-0",
        "-col-0",
        "row-start-0",
        "-col-end-0",
        "col-span-0",
        "row-span-0",
        // `-webkit-line-clamp` and `repeat()` require a positive count.
        "line-clamp-0",
        "grid-cols-0",
        "grid-rows-0",
        // `max-*` sizes accept `none`, not `auto`.
        "max-w-auto",
        "max-h-auto",
        "max-inline-auto",
        "max-block-auto",
        // `scroll-margin` does not accept percentages.
        "scroll-m-1/2",
        "-scroll-mt-1/3",
        // Conic gradient stops accept angles and percentages, not lengths.
        "mask-conic-from-4",
        "mask-conic-to-px",
        "mask-conic-from-0",
        // Bare numbers are written in decimal digits.
        "grow-inf",
        "opacity-NaN",
        "z-1e3",
    ] {
        assert!(
            resolve_utility(candidate, &theme).is_none(),
            "`{candidate}` must not resolve"
        );
    }
    let theme = theme_with_utility_namespaces();
    let resolved = resolve_utility("perspective-origin-corner", &theme).unwrap();
    assert!(all_declarations(&resolved).starts_with("perspective-origin:"));
    for candidate in ["mask-conic-from-50%", "mask-linear-from-4", "indent-1/2"] {
        assert!(
            resolve_utility(candidate, &theme).is_some(),
            "`{candidate}` must resolve"
        );
    }
}

#[test]
fn utilities_without_a_value_read_bare_theme_tokens() {
    let theme = theme();
    for (candidate, declaration) in [
        ("rounded", "border-radius:0.25rem;"),
        (
            "rounded-t",
            "border-top-left-radius:0.25rem;border-top-right-radius:0.25rem;",
        ),
        ("rounded-bl", "border-bottom-left-radius:0.25rem;"),
        ("blur", "--tw-blur:blur(8px);"),
        ("backdrop-blur", "--tw-backdrop-blur:blur(8px);"),
        (
            "shadow",
            "--tw-shadow:01px3px0var(--tw-shadow-color,rgb(000/0.1))",
        ),
        ("shadow/50", "--tw-shadow-alpha:50%;"),
        ("drop-shadow", "--tw-drop-shadow-size:drop-shadow("),
        ("drop-shadow/50", "--tw-drop-shadow-alpha:50%;"),
    ] {
        let resolved = resolve_utility(candidate, &theme)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            all_declarations(&resolved).contains(declaration),
            "`{candidate}` must declare {declaration}: {}",
            all_declarations(&resolved)
        );
    }
    for candidate in ["text-shadow", "inset-shadow", "rounded/50", "blur/50"] {
        assert!(
            resolve_utility(candidate, &theme).is_none(),
            "`{candidate}` must not resolve"
        );
    }

    let reset = parse_fixture_theme(
        &SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --radius: initial; --radius-sm: initial; --blur: initial; --shadow: initial; --drop-shadow: initial; }",
        )
        .unwrap(),
    )
    .unwrap();
    for candidate in [
        "rounded",
        "rounded-t",
        "rounded-sm",
        "blur",
        "backdrop-blur",
        "shadow",
        "drop-shadow",
    ] {
        assert!(
            resolve_utility(candidate, &reset).is_none(),
            "`{candidate}` must not resolve without its token"
        );
    }

    let custom = parse_fixture_theme(
        &SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --radius: 1rem; --backdrop-blur: 2px; --text-shadow: 0 1px 2px red; --inset-shadow: inset 0 1px 2px red; }",
        )
        .unwrap(),
    )
    .unwrap();
    for (candidate, declaration) in [
        ("rounded", "border-radius:var(--radius);"),
        (
            "backdrop-blur",
            "--tw-backdrop-blur:blur(var(--backdrop-blur));",
        ),
        (
            "text-shadow",
            "text-shadow:01px2pxvar(--tw-text-shadow-color,red);",
        ),
        ("text-shadow/50", "--tw-text-shadow-alpha:50%;"),
        (
            "inset-shadow",
            "--tw-inset-shadow:inset01px2pxvar(--tw-inset-shadow-color,red);",
        ),
    ] {
        let resolved = resolve_utility(candidate, &custom)
            .unwrap_or_else(|| panic!("expected `{candidate}` to resolve"));
        assert!(
            all_declarations(&resolved).contains(declaration),
            "`{candidate}` must declare {declaration}: {}",
            all_declarations(&resolved)
        );
    }
}
