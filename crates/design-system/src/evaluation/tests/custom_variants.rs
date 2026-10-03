//! User-authored variants and the CSS value functions they use.
//!
//! Pins that `@variant` blocks, wildcard selector variants, and container variants
//! resolve with the same slot syntax as built-ins, and that theme wildcards and typed
//! modifiers expand inside them.

use stillwind_test_support::assert_css_eq;
use stillwind_test_support::assertions::CssAssertions as _;

use super::*;
use crate::evaluation::builtins::metadata::COMPAT_COLOR_SUPPORT;
use crate::test_support::parse_fixture_theme;

#[test]
fn resolves_user_block_variants_with_the_same_slot_syntax_as_built_ins() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@variant pressed { &:active { @slot; } }\n@variant narrow { @media (width < 40rem) { @slot; } }\n@variant decorated { &::before { content: var(--content); @slot; } }",
    ).unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_eq!(
        resolve_variant(
            &parse_candidate("pressed:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate("&:active".to_owned()))
    );
    assert_eq!(
        resolve_variant(&parse_candidate("narrow:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Media("(width < 40rem)".to_owned()))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("decorated:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorRule {
            selector: "&::before".to_owned(),
            declarations: vec![VariantDeclaration {
                property: "content".to_owned(),
                value: "var(--content)".to_owned(),
            }],
        })
    );
}

#[test]
fn resolves_wildcard_selector_variants_from_application_css() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@variant state-* { &[data-state=--value()] { @slot; } }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_eq!(
        resolve_variant(
            &parse_candidate("state-open:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            "&[data-state=open]".to_owned()
        ))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("aria-expanded:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            "&[aria-expanded=\"true\"]".to_owned()
        ))
    );
}

#[test]
fn resolves_compat_color_value_function() {
    let source = SourceFile::new(
        FileId(1), None, None,
        "@theme { --color-brand: red; --breakpoint-narrow: 37rem; --breakpoint-wide: 80rem; }\n@utility tone-* { color: --compat-color(--value(--color-*), --alpha(--modifier())); }",
    ).unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    assert_css_eq!(
        resolve_utility("tone-brand/50", &theme)
            .unwrap()
            .declaration,
        "color: color-mix(in srgb, var(--color-brand) 50%, transparent);"
    );
    let utility = resolve_utility("tone-brand/50", &theme).unwrap();
    assert_eq!(utility.nested.len(), 1);
    assert_eq!(utility.nested[0].condition, COMPAT_COLOR_SUPPORT);
    assert!(
        utility.nested[0]
            .declaration
            .contains_css("color-mix(in oklab, var(--color-brand) 50%, transparent)")
    );

    let plain_source = SourceFile::new(
        FileId(2),
        None,
        None,
        "@utility plain-tone { color: --compat-color(red, 50%); }",
    )
    .unwrap();
    let plain_theme = parse_fixture_theme(&plain_source).unwrap();
    assert_css_eq!(
        resolve_utility("plain-tone", &plain_theme)
            .unwrap()
            .declaration,
        "color: color-mix(in srgb, red 50%, transparent);"
    );

    let nested_source = SourceFile::new(
        FileId(3), None, None,
        "@utility nested-tone-* { @media (width >= 1px) { color: --compat-color(red, --alpha(--modifier())); } }",
    ).unwrap();
    let nested_theme = parse_fixture_theme(&nested_source).unwrap();
    let nested = resolve_utility("nested-tone-brand/50", &nested_theme).unwrap();
    assert!(nested.nested.iter().any(|rule| {
        rule.at_rule == NestedUtilityAtRule::Supports
            && rule.declaration.contains("color-mix(in oklab")
    }));
}

#[test]
fn expands_theme_wildcards_in_nested_utility_rules() {
    let source = SourceFile::new(
        FileId(1), None, None,
        "@theme { --breakpoint-narrow: 37rem; --breakpoint-wide: 80rem; }\n@utility container { width: 100%; @media (width >= --value(--breakpoint-*)) { max-width: --value(--breakpoint-*); } }",
    ).unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    let utility = resolve_utility("container", &theme).unwrap();
    let conditions = utility
        .nested
        .iter()
        .map(|rule| rule.condition.as_str())
        .collect::<Vec<_>>();
    assert!(conditions.contains(&"(width >= 37rem)"));
    assert!(conditions.contains(&"(width >= 80rem)"));
}

#[test]
fn resolves_container_at_variants_from_css_registry() {
    let source =
        SourceFile::new(FileId(1), None, None, "@theme { --container-md: 40rem; }").unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    let candidate = parse_candidate("@min-md:flex").unwrap();
    assert_eq!(
        resolve_variant(&candidate.variants[0], &theme),
        Some(VariantApplication::Container("(width >= 40rem)".to_owned()))
    );
    for (candidate, condition) in [
        ("@md/sidebar:flex", "sidebar (width >= 40rem)"),
        ("@[48rem]/main:flex", "main (width >= 48rem)"),
        ("@max-[48rem]/main:flex", "main (width < 48rem)"),
    ] {
        let candidate = parse_candidate(candidate).unwrap();
        assert_eq!(
            resolve_variant(&candidate.variants[0], &theme),
            Some(VariantApplication::Container(condition.to_owned()))
        );
    }
}

#[test]
fn resolves_formal_variant_modifier_placeholder() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@variant rel-* { :where(.peer/{modifier}) ~ & { @slot; } }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    assert_eq!(
        resolve_variant(
            &parse_candidate("rel-checked/field:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            r":where(.peer\/field) ~ &".to_owned()
        ))
    );
}

#[test]
fn resolves_wildcard_conditional_variants_from_css() {
    let source = SourceFile::new(
        FileId(1), None, None,
        "@theme { --breakpoint-narrow: 40rem; }\n@variant narrow-* { @media (width < --value(--breakpoint-*)) { @slot; } }",
    ).unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_eq!(
        resolve_variant(&parse_candidate("min-md:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Media("(width >= 48rem)".to_owned()))
    );
    assert_eq!(
        resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("narrow-narrow:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::Media("(width < 40rem)".to_owned()))
    );
}
