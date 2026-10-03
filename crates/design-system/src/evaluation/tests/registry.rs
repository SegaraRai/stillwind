//! Registry metadata as the source of resolution behavior.
//!
//! Pins arbitrary-property order, animation option resolution, theme suffix metadata,
//! and the value domains used by theme namespaces.

use stillwind_test_support::assertions::css_tokens;
use stillwind_test_support::defaults::BUILTIN_REGISTRY_CSS;

use super::*;
use crate::parse_builtin_utility_definitions;
use crate::test_support::parse_fixture_theme;

#[test]
fn derives_arbitrary_property_order_from_builtin_css() {
    let transform = resolve_utility("transform-gpu", &theme())
        .expect("transform utility")
        .builtin_order
        .expect("built-in order");
    let margin = resolve_utility("[margin:0]", &theme())
        .expect("arbitrary margin")
        .builtin_order
        .expect("built-in order");
    let margin_left = resolve_utility("[margin-left:1px]", &theme())
        .expect("arbitrary margin-left")
        .builtin_order
        .expect("derived order");
    let all = resolve_utility("[all:unset]", &theme())
        .expect("arbitrary all")
        .builtin_order
        .expect("derived order");
    let background = resolve_utility("[background:red]", &theme())
        .expect("arbitrary background")
        .builtin_order
        .expect("derived order");
    let translate_variable = resolve_utility("[--tw-translate-x:1px]", &theme())
        .expect("positioned Tailwind variable")
        .builtin_order
        .expect("declared order");
    let unknown_tailwind_variable = resolve_utility("[--tw-unknown:1]", &theme())
        .expect("fallback Tailwind variable")
        .builtin_order
        .expect("declared order");
    assert!(margin < margin_left && margin_left < transform);
    assert!(translate_variable < transform);
    assert!(transform < all, "transform={transform}, all={all}");
    assert!(transform < unknown_tailwind_variable);
    assert!(unknown_tailwind_variable < background);
    assert_eq!(background, all);
}

#[test]
fn resolves_animation_options_from_css_registry_metadata() {
    let mut theme = theme();
    theme.tokens.extend(
        parse_fixture_theme(
            &SourceFile::new(
                FileId(2),
                None,
                None,
                "@theme { --duration-normal: 180ms; --ease-standard: cubic-bezier(.2, 0, 0, 1); }",
            )
            .unwrap(),
        )
        .unwrap()
        .tokens,
    );
    assert_eq!(
        resolve_animation_option("both", &theme).as_deref(),
        Some("animation-fill-mode:both;")
    );
    assert_eq!(
        resolve_animation_option("duration-normal", &theme).as_deref(),
        Some("animation-duration:var(--duration-normal);")
    );
    assert_eq!(
        resolve_animation_option("duration-120", &theme).map(|value| css_tokens(&value)),
        Some(css_tokens("animation-duration:120ms;"))
    );
    assert_eq!(
        resolve_animation_option("ease-standard", &theme).as_deref(),
        Some("animation-timing-function:var(--ease-standard);")
    );
    assert_eq!(
        resolve_animation_option("[transition-duration:75ms]", &theme).as_deref(),
        Some("animation-duration:75ms;")
    );
}

#[test]
fn resolves_related_theme_tokens_through_generic_suffix_metadata() {
    let theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(2),
            None,
            None,
            "@theme { --text-copy: 1rem; --text-copy--line-height: 1.5; }",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        resolve_utility("text-copy", &theme).map(|utility| css_tokens(&utility.declaration)),
        Some(css_tokens(
            "font-size:var(--text-copy);line-height:var(--tw-leading,var(--text-copy--line-height));"
        ))
    );
}

#[test]
fn simple_utility_domains_follow_csswg_formal_syntax() {
    let mut checked = BTreeSet::new();
    let mut failures = Vec::new();
    for utility in parse_builtin_utility_definitions(BUILTIN_REGISTRY_CSS) {
        for declaration in utility.body.split(';') {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let property = property.trim();
            if property.starts_with('-') || property.contains('{') {
                continue;
            }
            for domain in direct_value_domains(value) {
                let Some(expected_terms) = domain.csswg_formal_terms() else {
                    continue;
                };
                let mut visited = BTreeSet::new();
                let Some(syntax) = expanded_csswg_syntax(property, &mut visited) else {
                    failures.push(format!(
                        "{property} in @utility {} has no Webref formal syntax",
                        utility.name
                    ));
                    continue;
                };
                let terms = formal_terms(&syntax);
                // A named production such as `<grid-line>` needs
                // property-specific interpretation. Those compound
                // grammars are detected from their syntax rather than a
                // per-property exception list.
                if has_complex_csswg_grammar(&syntax, &terms) {
                    continue;
                }
                if expected_terms
                    .iter()
                    .any(|expected| terms.contains(expected))
                {
                    checked.insert(format!("{property} [{domain:?}]"));
                } else {
                    failures.push(format!(
                            "{property} in @utility {} uses [{domain:?}], but CSSWG declares {syntax:?}",
                            utility.name
                        ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "built-in simple value domains diverge from CSSWG formal syntax:\n{}",
        failures.join("\n")
    );
    assert!(
        checked.len() >= 20,
        "the registry-wide audit unexpectedly covered only {} declarations: {checked:?}",
        checked.len()
    );
}

#[test]
fn every_tailwind_theme_namespace_has_a_typed_arbitrary_value_domain() {
    for namespace in [
        ThemeNamespace::Animate,
        ThemeNamespace::AspectRatio,
        ThemeNamespace::Blur,
        ThemeNamespace::Breakpoint,
        ThemeNamespace::Color,
        ThemeNamespace::Container,
        ThemeNamespace::DropShadow,
        ThemeNamespace::Ease,
        ThemeNamespace::Font,
        ThemeNamespace::FontStretch,
        ThemeNamespace::FontWeight,
        ThemeNamespace::InsetShadow,
        ThemeNamespace::Leading,
        ThemeNamespace::Perspective,
        ThemeNamespace::Radius,
        ThemeNamespace::Shadow,
        ThemeNamespace::Spacing,
        ThemeNamespace::Text,
        ThemeNamespace::TextShadow,
        ThemeNamespace::Tracking,
        ThemeNamespace::ZIndex,
        ThemeNamespace::Duration,
    ] {
        assert!(
            !theme_arbitrary_types(namespace, &theme()).is_empty(),
            "{namespace:?} must not fall back to an unchecked arbitrary value"
        );
    }
}
