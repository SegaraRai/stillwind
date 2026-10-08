//! Conditional declarations, relationships, and shared value families.
//!
//! Pins support and media conditions authored in CSS utilities, Tailwind relationship
//! primitives, numeric and CSS-wide keyword domains, and the compact spacing families
//! that mix negative and fractional values.

use stillwind_test_support::{CssAssertions as _, assert_css_eq, css_tokens};

use super::*;
use crate::parse_utility_definitions;
use crate::theme::ThemeBuilder;

#[test]
fn preserves_supports_and_media_conditions_from_css_utilities() {
    let theme = theme();
    let outline = resolve_utility("outline-hidden", &theme).unwrap();
    assert!(outline.nested.iter().any(|rule| {
        css_tokens(&rule.condition) == css_tokens("(forced-colors:active)")
            && rule.declaration.contains_css("outline-offset:2px;")
            && rule
                .declaration
                .contains_css("outline:2px solid transparent;")
    }));
}

#[test]
fn resolves_tailwind_relationship_compatibility_primitives() {
    let theme = theme();

    for candidate in [
        "group",
        "peer",
        "group-hover:flex",
        "peer-checked:flex",
        "@sm:flex",
        "@max-sm:flex",
        "@[10rem]:flex",
    ] {
        assert!(
            is_tailwind_relationship_candidate(candidate, &theme),
            "{candidate}"
        );
    }
    assert!(!is_tailwind_relationship_candidate("max-w-lg", &theme));
    assert_eq!(
        compatibility_marker_class("group", &theme).as_deref(),
        Some("group")
    );
    assert_eq!(
        compatibility_marker_class("peer/field", &theme).as_deref(),
        Some("peer/field")
    );
    assert_css_eq!(
        resolve_utility("@container", &theme).unwrap().declaration,
        "container-type:inline-size;"
    );
    assert_css_eq!(
        resolve_utility("@container/sidebar", &theme)
            .unwrap()
            .declaration,
        "container-name:sidebar;container-type:inline-size;"
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("group-hover:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorMedia {
            selector: "&:is(:where(.group):hover *)".to_owned(),
            condition: "(hover: hover)".to_owned(),
        })
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("peer-checked:flex").unwrap().variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            "&:is(:where(.peer):checked ~ *)".to_owned()
        ))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate("group-[&[data-open]]:flex")
                .unwrap()
                .variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorTemplate(
            "&:is(:where(.group)[data-open] *)".to_owned()
        ))
    );
    assert_eq!(
        resolve_variant(
            &parse_candidate(r"group-hover/menu\:open:flex")
                .unwrap()
                .variants[0],
            &theme
        ),
        Some(VariantApplication::SelectorMedia {
            selector: r"&:is(:where(.group\/menu\:open):hover *)".to_owned(),
            condition: "(hover: hover)".to_owned(),
        })
    );
    assert_eq!(
        resolve_variant(&parse_candidate("@sm:flex").unwrap().variants[0], &theme),
        Some(VariantApplication::Container("(width >= 24rem)".to_owned()))
    );
}

#[test]
fn resolves_numeric_css_domains_and_default_filter_tokens() {
    let theme = theme();
    let custom_theme = ThemeBuilder {
        utilities: parse_utility_definitions("@utility tab-* { tab-size: --value(integer); }"),
        ..Default::default()
    }
    .finish();

    assert_css_eq!(
        resolve_utility("tab-8", &custom_theme).unwrap().declaration,
        "tab-size: 8;"
    );
    assert_css_eq!(
        resolve_utility("blur-sm", &theme).unwrap().declaration,
        "--tw-blur:blur(var(--blur-sm));filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
    );
}

#[test]
fn resolves_theme_shadow_opacity_with_a_relative_color_function() {
    let shadow = resolve_utility("inset-shadow-sm/50", &theme()).unwrap();

    assert!(
        shadow
            .declaration
            .contains_css("--tw-inset-shadow-alpha:50%;")
    );
    assert!(
            shadow.declaration.contains_css(
                "--tw-inset-shadow:inset 0 2px 4px var(--tw-inset-shadow-color, oklab(from rgb(0 0 0 / 0.05) l a b / 50%));"
            ),
            "{}",
            shadow.declaration
        );
    assert!(shadow.declaration.contains_css(
            "box-shadow:var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);"
        ));
    assert!(resolve_utility("inset-shadow-sm/not-an-opacity", &theme()).is_none());
}

#[test]
fn keeps_css_wide_color_keywords_in_the_declared_utility_domain() {
    let theme = theme();

    assert!(resolve_utility("divide-inherit/50", &theme).is_some());
    assert!(resolve_utility("shadow-inherit", &theme).is_some());
    assert!(resolve_utility("shadow-inherit/50", &theme).is_none());
    assert!(resolve_utility("inset-shadow-inherit/50", &theme).is_none());
}

#[test]
fn resolves_css_wide_keywords_for_property_typed_built_ins() {
    for candidate in [
        "contain-[initial]",
        "font-features-[initial]",
        "object-[initial]",
        "will-change-[initial]",
        "zoom-[initial]",
    ] {
        assert!(
            resolve_utility(candidate, &theme()).is_some(),
            "{candidate} must resolve through its exact CSS property grammar"
        );
    }
}

#[test]
fn resolves_compact_spacing_families_with_negative_and_fractional_values() {
    let theme = theme();

    assert_css_eq!(
        resolve_utility("left-auto", &theme).unwrap().declaration,
        "left:auto;"
    );
    assert!(
        resolve_utility("-left-auto", &theme).is_none(),
        "negative keyword admissibility must come from the CSS definition"
    );
    assert_css_eq!(
        resolve_utility("-left-4", &theme).unwrap().declaration,
        "left:calc(var(--spacing) * -4);"
    );
    assert_css_eq!(
        resolve_utility("scroll-px-4", &theme).unwrap().declaration,
        "scroll-padding-inline:calc(var(--spacing) * 4);"
    );
    assert_css_eq!(
        resolve_utility("-left-1/2", &theme).unwrap().declaration,
        "left:calc(1 / 2 * 100% * -1);"
    );
    assert_css_eq!(
        resolve_utility("-bottom-full", &theme).unwrap().declaration,
        "bottom:-100%;"
    );
}
