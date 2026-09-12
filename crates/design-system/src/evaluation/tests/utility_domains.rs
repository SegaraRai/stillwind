//! Resolution coverage of the upstream utility catalog.
//!
//! Pins that every upstream static and functional root resolves from the CSS registry,
//! that namespace resets are respected, and that typed branches prune only the
//! declarations that do not match a candidate.

use super::*;

#[test]
fn resolves_every_upstream_static_utility_from_the_css_registry() {
    let catalog =
        include_str!("../../../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
    let candidates = catalog_section(catalog, "## Static utilities");
    let theme = theme();

    let unresolved = candidates
        .into_iter()
        .filter(|candidate| resolve_utility(candidate, &theme).is_none())
        .collect::<Vec<_>>();

    assert!(
        unresolved.is_empty(),
        "unresolved static utilities: {unresolved:?}"
    );
}

#[test]
fn resolves_every_upstream_functional_root_with_an_arbitrary_value() {
    let catalog =
        include_str!("../../../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
    let roots = catalog_section(catalog, "## Functional utility roots");
    let theme = theme();

    let unresolved = roots
        .into_iter()
        .map(|root| format!("{root}-[initial]"))
        .filter(|candidate| resolve_utility(candidate, &theme).is_none())
        .collect::<Vec<_>>();

    assert!(
        unresolved.is_empty(),
        "unresolved functional utility candidates: {unresolved:?}"
    );
}

#[test]
fn resolves_every_upstream_static_variant_from_the_css_registry() {
    let catalog =
        include_str!("../../../../../fixtures/tailwind-v4/catalog/direct-registrations.md");
    let variants = catalog_section(catalog, "## Static variants");
    let theme = theme();

    let unresolved = variants
        .into_iter()
        .filter(|name| {
            let candidate =
                parse_candidate(&format!("{name}:flex")).expect("catalog variants must parse");
            resolve_variant(&candidate.variants[0], &theme).is_none()
        })
        .collect::<Vec<_>>();

    assert!(
        unresolved.is_empty(),
        "unresolved static variants: {unresolved:?}"
    );
}

#[test]
fn preserves_a_nested_selector_from_a_css_utility() {
    let resolved = resolve_utility("divide-x-[2px]", &theme()).unwrap();

    assert_eq!(
        resolved.declaration,
        "border-inline-end-style:var(--tw-border-style);border-inline-end-width:2px;"
    );
    assert_eq!(
        resolved.selector.as_deref(),
        Some(":where(&>:not(:last-child))")
    );
}

#[test]
fn resolves_fixture_style_static_theme_and_arbitrary_candidates() {
    let theme = theme();
    assert_eq!(
        resolve_utility("!flex", &theme).unwrap().declaration,
        "display:flex!important;"
    );
    assert_eq!(
        resolve_utility("-mt-4", &theme).unwrap().declaration,
        "margin-top:calc(var(--spacing) * -4);"
    );
    let background = resolve_utility("bg-brand/50", &theme).unwrap();
    assert_eq!(
        background.declaration,
        "background-color:color-mix(in srgb, red 50%, transparent);"
    );
    assert!(background.nested.iter().any(|rule| {
        rule.at_rule == NestedUtilityAtRule::Supports
            && rule
                .declaration
                .contains("background-color:color-mix(in oklab")
            && rule.declaration.contains("color-mix(in oklab")
    }));
    assert_eq!(
        resolve_utility("w-(--sidebar)", &theme)
            .unwrap()
            .declaration,
        "width:var(--sidebar);"
    );
    assert_eq!(
        resolve_utility("[color:rebeccapurple]", &theme)
            .unwrap()
            .declaration,
        "color:rebeccapurple;"
    );
}

#[test]
fn respects_tailwind_theme_namespace_resets_when_resolving_utilities() {
    let default_theme = theme();
    assert_eq!(
        resolve_utility("bg-red-500", &default_theme)
            .expect("the vendored Tailwind theme provides red-500")
            .declaration,
        "background-color:var(--color-red-500);"
    );

    let reset_theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-*: initial; --color-brand: rebeccapurple; }",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(resolve_utility("bg-red-500", &reset_theme).is_none());
    assert_eq!(
        resolve_utility("bg-brand", &reset_theme)
            .unwrap()
            .declaration,
        "background-color:var(--color-brand);"
    );
    assert_eq!(
        resolve_utility("bg-current", &reset_theme)
            .unwrap()
            .declaration,
        "background-color:currentcolor;"
    );
    assert_eq!(
        resolve_utility("bg-transparent", &reset_theme)
            .unwrap()
            .declaration,
        "background-color:transparent;"
    );
}

#[test]
fn resolves_native_mask_conic_angles() {
    let resolved = resolve_utility("mask-conic-45", &theme()).expect("mask conic angle");
    assert!(
        resolved
            .declaration
            .contains("--tw-mask-conic-position:45deg;"),
        "{resolved:?}"
    );
    let negative = resolve_utility("-mask-conic-45", &theme()).expect("negative mask conic angle");
    assert!(
        negative
            .declaration
            .contains("--tw-mask-conic-position:-45deg;"),
        "{negative:?}"
    );
}

#[test]
fn resolves_gradient_interpolation_modifiers_in_the_css_registry() {
    let default = resolve_utility("bg-conic-45", &theme()).expect("default conic");
    assert!(
        default
            .declaration
            .contains("--tw-gradient-position:from 45deg in oklab;"),
        "{default:?}"
    );
    let interpolated =
        resolve_utility("bg-conic-45/increasing", &theme()).expect("interpolated conic");
    assert!(
        interpolated
            .declaration
            .contains("from 45deg in oklch increasing hue")
    );
}

#[test]
fn resolves_css_defined_color_and_gradient_utilities() {
    let theme = theme();
    assert_eq!(
        resolve_utility("fill-brand/50", &theme)
            .unwrap()
            .declaration,
        "fill:color-mix(in srgb, red 50%, transparent);"
    );
    assert_eq!(
        resolve_utility("from-brand", &theme).unwrap().declaration,
        "--tw-gradient-from:var(--color-brand);--tw-gradient-stops:var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));"
    );
    assert_eq!(
        resolve_utility("bg-linear-to-r", &theme)
            .unwrap()
            .declaration,
        "--tw-gradient-position:to right;background-image:linear-gradient(var(--tw-gradient-stops));"
    );
}

#[test]
fn resolves_ambiguous_backgrounds_through_typed_utility_branches() {
    let theme = theme();
    assert_eq!(
        resolve_utility("bg-[10px]", &theme).unwrap().declaration,
        "background-position:10px;"
    );
    assert_eq!(
        resolve_utility("bg-[20%_30%]", &theme).unwrap().declaration,
        "background-position:20% 30%;"
    );
    assert_eq!(
        resolve_utility("bg-[10px_auto]", &theme)
            .unwrap()
            .declaration,
        "background-size:10px auto;"
    );
    assert_eq!(
        resolve_utility("bg-[var(--surface)]", &theme)
            .unwrap()
            .declaration,
        "background-color:var(--surface);"
    );
    assert_eq!(
        resolve_utility("bg-[color:rebeccapurple]", &theme)
            .unwrap()
            .declaration,
        "background-color:rebeccapurple;"
    );
    assert_eq!(
        resolve_utility("bg-[image:linear-gradient(red,blue)]", &theme)
            .unwrap()
            .declaration,
        "background-image:linear-gradient(red,blue);"
    );
    assert_eq!(
        resolve_utility("bg-[position:center_20%]", &theme)
            .unwrap()
            .declaration,
        "background-position:center 20%;"
    );
    assert_eq!(
        resolve_utility("bg-[bg-size:24rem_auto]", &theme)
            .unwrap()
            .declaration,
        "background-size:24rem auto;"
    );
}

#[test]
fn prunes_only_incompatible_declarations_from_a_typed_utility_body() {
    let source = SourceFile::new(
        FileId(1),
        "theme.css",
        r"@utility token-* {
  color: --value([color]);
  width: --value([length]);
}",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_eq!(
        resolve_utility("token-[color:rebeccapurple]", &theme)
            .unwrap()
            .declaration,
        "color: rebeccapurple;"
    );
    assert_eq!(
        resolve_utility("token-[length:12px]", &theme)
            .unwrap()
            .declaration,
        "width: 12px;"
    );
}

#[test]
fn rejects_untyped_or_invalid_modifiers_instead_of_ignoring_them() {
    let theme = theme();

    assert!(resolve_utility("bg-red-500/not-an-opacity", &theme).is_none());
    assert!(resolve_utility("bg-[image:url(/hero.svg)]/50", &theme).is_none());
    let background = resolve_utility("bg-red-500/50", &theme).unwrap();
    assert_eq!(
        background.declaration,
        "background-color:color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);"
    );
    assert!(background.nested.iter().any(|rule| {
        rule.at_rule == NestedUtilityAtRule::Supports
            && rule
                .declaration
                .contains("background-color:color-mix(in oklab")
            && rule.declaration.contains("color-mix(in oklab")
    }));
}

#[test]
fn resolves_typed_modifier_domains_without_accepting_other_types() {
    let source = SourceFile::new(
        FileId(1),
        "theme.css",
        r"@utility prose-* {
  font-size: --value(--text-*);
  line-height: --modifier(--leading-*, --spacing(), [length]);
}",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_eq!(
        resolve_utility("prose-sm/6", &theme).unwrap().declaration,
        "font-size: var(--text-sm);\n  line-height: calc(var(--spacing) * 6);"
    );
    assert!(resolve_utility("prose-sm/[color:rebeccapurple]", &theme).is_none());
}
