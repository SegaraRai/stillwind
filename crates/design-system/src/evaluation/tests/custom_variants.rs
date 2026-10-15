//! User-authored variants and the CSS value functions they use.
//!
//! Pins that `@variant` blocks, wildcard selector variants, and container variants
//! resolve with the same slot syntax as built-ins, and that theme wildcards and typed
//! modifiers expand inside them.

use lean_string::ToLeanStr;

use stillwind_test_support::{CssAssertions as _, assert_css_eq};

use super::*;
use crate::evaluation::builtins::COMPAT_COLOR_SUPPORT;
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
        Some(VariantApplication::SelectorTemplate(
            "&:active".to_lean_str()
        ))
    );
    assert_eq!(
        resolve_variant(&parse_candidate("narrow:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Media("(width < 40rem)".to_lean_str()))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("decorated:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorRule {
            selector: "&::before".to_lean_str(),
            declarations: vec![VariantDeclaration {
                property: "content".to_lean_str(),
                value: "var(--content)".to_lean_str(),
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
            "&[data-state=open]".to_lean_str()
        ))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("aria-expanded:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            "&[aria-expanded=\"true\"]".to_lean_str()
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
        "color: color-mix(in srgb, red 50%, transparent);"
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
        "color: color-mix(in oklab, red 50%, transparent);"
    );

    let nested_source = SourceFile::new(
        FileId(3), None, None,
        "@utility nested-tone-* { @media (width >= 1px) { color: --compat-color(red, --alpha(--modifier())); } }",
    ).unwrap();
    let nested_theme = parse_fixture_theme(&nested_source).unwrap();
    let nested = resolve_utility("nested-tone-brand/50", &nested_theme).unwrap();
    assert!(nested.nested.iter().any(|rule| {
        rule.at_rule == NestedUtilityAtRule::Media
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
        Some(VariantApplication::Container(
            "(width >= 40rem)".to_lean_str()
        ))
    );
    for (candidate, condition) in [
        ("@md/sidebar:flex", "sidebar (width >= 40rem)"),
        ("@[48rem]/main:flex", "main (width >= 48rem)"),
        ("@max-[48rem]/main:flex", "main (width < 48rem)"),
    ] {
        let candidate = parse_candidate(candidate).unwrap();
        assert_eq!(
            resolve_variant(&candidate.variants[0], &theme),
            Some(VariantApplication::Container(condition.to_lean_str()))
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
            r":where(.peer\/field) ~ &".to_lean_str()
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
        Some(VariantApplication::Media("(width >= 48rem)".to_lean_str()))
    );
    assert_eq!(
        resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Container(
            "(width >= 24rem)".to_lean_str()
        ))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("narrow-narrow:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::Media("(width < 40rem)".to_lean_str()))
    );
}

#[test]
fn shorthand_variants_resolve_at_rules_and_selectors() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@custom-variant pm (@media print);\n@custom-variant grid-ok (@supports (display: grid));\n@custom-variant dim (&:where(.dark, .dark *));",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    let resolve =
        |candidate: &str| resolve_variant(&parse_candidate(candidate).unwrap().variants[0], &theme);

    for (candidate, expected) in [
        ("pm:flex", VariantApplication::Media("print".to_lean_str())),
        (
            "not-pm:flex",
            VariantApplication::Media("not print".to_lean_str()),
        ),
        (
            "grid-ok:flex",
            VariantApplication::Supports("(display: grid)".to_lean_str()),
        ),
        (
            "dim:flex",
            VariantApplication::SelectorTemplate("&:where(.dark, .dark *)".to_lean_str()),
        ),
        (
            "not-dim:flex",
            VariantApplication::SelectorTemplate("&:not(:where(.dark, .dark *))".to_lean_str()),
        ),
    ] {
        assert_eq!(resolve(candidate), Some(expected), "{candidate}");
    }
}
