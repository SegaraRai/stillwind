//! Registry metadata as the source of resolution behavior.
//!
//! Pins arbitrary-property order, context registries, theme suffix metadata, and the
//! value domains used by theme namespaces.

use lean_string::ToLeanString;

use stillwind_source::JoinText;
use stillwind_test_support::{BUILTIN_REGISTRY_CSS, css_tokens};

use super::*;
use crate::test_support::parse_fixture_theme;
use crate::{RegistryContext, parse_builtin_utility_definitions};

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
fn resolves_animation_options_through_the_animation_context() {
    let theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(2),
            None,
            None,
            "@theme { --duration-normal: 180ms; --ease-standard: cubic-bezier(.2, 0, 0, 1); }",
        )
        .unwrap(),
    )
    .unwrap();
    let animation = theme.context(RegistryContext::Animation);
    let declaration =
        |raw: &str| resolve_utility(raw, animation).map(|utility| css_tokens(&utility.declaration));
    assert_eq!(
        declaration("both"),
        Some(css_tokens("animation-fill-mode:both;"))
    );
    assert_eq!(
        declaration("duration-normal"),
        Some(css_tokens("animation-duration:var(--duration-normal);"))
    );
    assert_eq!(
        declaration("ease-standard"),
        Some(css_tokens(
            "animation-timing-function:var(--ease-standard);"
        ))
    );
    assert_eq!(
        declaration("[animation-duration:75ms]"),
        Some(css_tokens("animation-duration:75ms;"))
    );
    for rejected in ["[transition-duration:75ms]", "opacity-0", "flex"] {
        assert_eq!(declaration(rejected), None, "{rejected}");
    }
    assert!(
        resolve_utility("duration-150", &theme)
            .expect("default duration")
            .declaration
            .contains("transition-duration:")
    );
}

#[test]
fn registry_blocks_replace_definitions_of_every_layer_in_their_contexts() {
    let theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(2),
            None,
            None,
            "@utility tone-* { color: --value([color]); }
@utility keep { color: green; }
@stillwind-registry keyframe, view-transition {
  @stillwind-undef-utility tone-*;
  @utility tone-* { background-color: --value([color]); }
  @stillwind-undef-variant hover;
}",
        )
        .unwrap(),
    )
    .unwrap();
    let declaration = |raw: &str, theme: &ThemeIndex| {
        resolve_utility(raw, theme).map(|utility| css_tokens(&utility.declaration))
    };
    assert_eq!(
        declaration("tone-[red]", &theme),
        Some(css_tokens("color:red;"))
    );
    for context in [RegistryContext::Keyframe, RegistryContext::ViewTransition] {
        let context = theme.context(context);
        assert_eq!(
            declaration("tone-[red]", context),
            Some(css_tokens("background-color:red;"))
        );
        assert_eq!(
            declaration("keep", context),
            Some(css_tokens("color:green;"))
        );
        assert!(
            context
                .registry()
                .variants()
                .iter()
                .all(|variant| variant.name != "hover")
        );
    }
    assert!(
        theme
            .registry()
            .variants()
            .iter()
            .any(|variant| variant.name == "hover")
    );
    assert_eq!(
        declaration("tone-[red]", theme.context(RegistryContext::Animation)),
        None
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
                    failures.push(
                        format_args!(
                            "{property} in @utility {} has no Webref formal syntax",
                            utility.name
                        )
                        .to_lean_string(),
                    );
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
                    checked.insert(format_args!("{property} [{domain:?}]").to_lean_string());
                } else {
                    failures.push(format_args!(
                            "{property} in @utility {} uses [{domain:?}], but CSSWG declares {syntax:?}",
                            utility.name
                        ).to_lean_string());
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "built-in simple value domains diverge from CSSWG formal syntax:\n{}",
        failures.join_text("\n")
    );
    assert!(
        checked.len() >= 20,
        "the registry-wide audit unexpectedly covered only {} declarations: {checked:?}",
        checked.len()
    );
}

#[test]
fn every_tailwind_theme_namespace_has_a_typed_arbitrary_value_domain() {
    let theme = theme();
    let namespaces = theme.namespaces().iter().collect::<Vec<_>>();
    assert!(namespaces.len() >= 22, "{namespaces:?}");
    for namespace in namespaces {
        assert!(
            !namespace.types.is_empty(),
            "{} must not fall back to an unchecked arbitrary value",
            namespace.name
        );
    }
}
