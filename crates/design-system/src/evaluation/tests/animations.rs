//! Placeholder and value-domain families exercised through real built-ins.
//!
//! Pins pseudo-element colors, gradient direction and interpolation, size and filter
//! keywords, and the rule that application CSS may override any compatibility utility
//! name.

use stillwind_test_support::assert_css_eq;
use stillwind_test_support::assertions::CssAssertions as _;

use super::*;
use crate::test_support::parse_fixture_theme;

#[test]
fn resolves_placeholder_colors_as_typed_pseudo_element_utilities() {
    let resolved = resolve_utility("placeholder-current/50", &theme()).unwrap();
    assert_eq!(resolved.selector.as_deref(), Some("&::placeholder"));
    assert!(
        resolved.declaration.contains_css("color:currentcolor;"),
        "{resolved:#?}"
    );
    assert!(resolved.nested.iter().any(|rule| {
        rule.declaration
            .contains_css("color-mix(in oklab, currentcolor 50%, transparent)")
    }));
    assert!(resolve_utility("placeholder-[length:1rem]", &theme()).is_none());
}

#[test]
fn resolves_gradient_direction_and_interpolation_from_css_maps() {
    let resolved = resolve_utility("bg-linear-to-r/oklch", &theme()).unwrap();
    assert!(
        resolved
            .declaration
            .contains_css("--tw-gradient-position:to right;")
    );
    assert!(
        resolved
            .nested
            .iter()
            .any(|nested| nested.declaration.contains_css("to right in oklch"))
    );
    let radial = resolve_utility("bg-radial/hsl", &theme()).unwrap();
    assert!(
        radial
            .declaration
            .contains_css("--tw-gradient-position:in hsl;")
    );
}

#[test]
fn resolves_size_as_a_functional_fraction_or_keyword() {
    let fractional = resolve_utility("size-7/12", &theme()).unwrap();
    assert!(
        fractional
            .declaration
            .contains_css("width:calc(7 / 12 * 100%);height:calc(7 / 12 * 100%);")
    );
    assert_css_eq!(
        resolve_utility("size-fit", &theme()).unwrap().declaration,
        "width:fit-content;height:fit-content;"
    );
}

#[test]
fn resolves_filter_and_perspective_keywords() {
    let theme = theme();
    assert_css_eq!(
        resolve_utility("blur-none", &theme).unwrap().declaration,
        "--tw-blur:;filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);"
    );
    assert_css_eq!(
        resolve_utility("perspective-dramatic", &theme)
            .unwrap()
            .declaration,
        "perspective:var(--perspective-dramatic);"
    );
    assert_css_eq!(
        resolve_utility("perspective-origin-top-left", &theme)
            .unwrap()
            .declaration,
        "perspective-origin:top left;"
    );
}

#[test]
fn lets_user_css_override_compatibility_fallback_utility_names() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "@utility size-* { inline-size: --value(--spacing-*); block-size: --value(--spacing-*); }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_css_eq!(
        resolve_utility("size-4", &theme).unwrap().declaration,
        "inline-size: calc(var(--spacing) * 4); block-size: calc(var(--spacing) * 4);"
    );
}

#[test]
fn resolves_animation_size_and_grid_columns_from_css_value_domains() {
    let theme = theme();

    assert_css_eq!(
        resolve_utility("animate-spin", &theme).unwrap().declaration,
        "animation:var(--animate-spin);"
    );
    assert_css_eq!(
        resolve_utility("size-[12px]", &theme).unwrap().declaration,
        "width:12px;height:12px;"
    );
    assert_css_eq!(
        resolve_utility("grid-cols-13", &theme).unwrap().declaration,
        "grid-template-columns:repeat(13,minmax(0,1fr));"
    );
}

#[test]
fn resolves_translate_and_scale_fallbacks_from_css_value_domains() {
    let theme = theme();

    assert_css_eq!(
        resolve_utility("translate-x-[12px]", &theme)
            .unwrap()
            .declaration,
        "--tw-translate-x:12px;translate:var(--tw-translate-x) var(--tw-translate-y);"
    );
    assert_css_eq!(
        resolve_utility("-translate-y-[12px]", &theme)
            .unwrap()
            .declaration,
        "--tw-translate-y:-12px;translate:var(--tw-translate-x) var(--tw-translate-y);"
    );
    assert_css_eq!(
        resolve_utility("scale-[0.8]", &theme).unwrap().declaration,
        "scale:0.8;"
    );
}
