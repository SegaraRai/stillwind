//! Overloaded CSS definitions and compatibility output.
//!
//! Pins that `@utility` definitions use the same value domains as `@stillwind-utility`,
//! that overloads are tried in reverse source order, that CSS fallbacks and extra
//! selector targets survive merging in registration order, that definitions of one name
//! add to each other as in Tailwind, and that color alpha emits both the sRGB fallback
//! and the oklab upgrade.

use stillwind_test_support::{CssAssertions as _, assert_css_eq};

use super::*;
use crate::test_support::parse_fixture_theme;

/// Parses project CSS on top of the standard built-ins.
fn project(css: &str) -> ThemeIndex {
    parse_fixture_theme(&SourceFile::new(FileId(1), None, None, css).unwrap()).unwrap()
}

#[test]
fn definitions_of_one_name_add_to_each_other_in_registration_order() {
    let theme = project(
        r"@utility container { padding-inline: 1rem; }
@utility duration-* { animation-duration: calc(--value(integer) * 1ms); }",
    );
    assert_css_eq!(
        resolve_utility("container", &theme).unwrap().declaration,
        "width: 100%; padding-inline: 1rem;"
    );
    assert_css_eq!(
        resolve_utility("duration-300", &theme).unwrap().declaration,
        "--tw-duration: calc(300 * 1ms); transition-duration: calc(300 * 1ms); \
         animation-duration: calc(300 * 1ms);"
    );
}

#[test]
fn an_exact_definition_makes_its_complete_candidate_static() {
    // As in Tailwind, a static utility hides the functional ones of its root, so the
    // `@stillwind-utility shadow-*` does not apply to `shadow-lg`.
    let theme = project(
        r"@utility shadow-lg { box-shadow: none; }
@utility tab-1\/2 { tab-size: 2; }",
    );
    assert_css_eq!(
        resolve_utility("shadow-lg", &theme).unwrap().declaration,
        "box-shadow: none;"
    );
    // An escaped name includes the modifier, which it then consumes.
    assert_css_eq!(
        resolve_utility("tab-1/2", &theme).unwrap().declaration,
        "tab-size: 2;"
    );
}

#[test]
fn functional_definitions_join_their_root_and_honor_stop_and_fallback() {
    let theme = project(
        r"@utility duration-300 (functional) { animation-duration: 300ms; }
@utility -probe-4\/mirror (functional) { inset: -1rem; }
@utility duration-\[1s\] (functional, stop) {}
@utility duration-\[1s\] (functional) { animation-duration: 1s; }
@utility probe-x (functional, fallback) { color: red; }",
    );
    assert_css_eq!(
        resolve_utility("duration-300", &theme).unwrap().declaration,
        "--tw-duration: calc(300 * 1ms); transition-duration: calc(300 * 1ms); \
         animation-duration: 300ms;"
    );
    assert_css_eq!(
        resolve_utility("-probe-4/mirror", &theme)
            .unwrap()
            .declaration,
        "inset: -1rem;"
    );
    // `(stop)` keeps the `@stillwind-utility` result and skips the later registration.
    assert_css_eq!(
        resolve_utility("duration-[1s]", &theme)
            .unwrap()
            .declaration,
        "--tw-duration: 1s; transition-duration: 1s;"
    );
    // A fallback applies only when nothing else resolves.
    assert_css_eq!(
        resolve_utility("probe-x", &theme).unwrap().declaration,
        "color: red;"
    );
    assert!(resolve_utility("probe-y", &theme).is_none());
}

#[test]
fn resolves_user_utilities_with_the_same_value_domains_as_built_ins() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"@theme { --color-brand: red; }
@utility tone-* { color: --value(--color-*); }
@utility rail-* { inline-size: --value(--spacing()); }
@utility -rail-* { inline-size: calc(--value(--spacing()) * -1); }
@utility offset-* { inset: --value-map(auto: auto, full: 100%); }
@utility -offset-* { inset: calc(--value-map(full: 100%) * -1); }
@utility mask-* { mask-size: --value([length]); }
@utility alpha-* { opacity: --modifier(); }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_css_eq!(
        resolve_utility("tone-brand", &theme).unwrap().declaration,
        "color: var(--color-brand);"
    );
    assert_css_eq!(
        resolve_utility("rail-4", &theme).unwrap().declaration,
        "inline-size: calc(var(--spacing) * 4);"
    );
    assert_css_eq!(
        resolve_utility("-rail-4", &theme).unwrap().declaration,
        "inline-size: calc(var(--spacing) * -4);"
    );
    assert_css_eq!(
        resolve_utility("offset-auto", &theme).unwrap().declaration,
        "inset: auto;"
    );
    assert!(resolve_utility("-offset-auto", &theme).is_none());
    assert_css_eq!(
        resolve_utility("-offset-full", &theme).unwrap().declaration,
        "inset: -100%;"
    );
    assert_css_eq!(
        resolve_utility("mask-[length:12px]", &theme)
            .unwrap()
            .declaration,
        "mask-size: 12px;"
    );
    assert_css_eq!(
        resolve_utility("alpha-brand/50", &theme)
            .unwrap()
            .declaration,
        "opacity: 50;"
    );
}

#[test]
fn tries_overloaded_css_utility_definitions_in_reverse_source_order() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"@utility token-* { width: --value([length]); }
@utility token-* { color: --value([color]); }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();

    assert_css_eq!(
        resolve_utility("token-[color:rebeccapurple]", &theme)
            .unwrap()
            .declaration,
        "color: rebeccapurple;"
    );
    assert_css_eq!(
        resolve_utility("token-[length:12px]", &theme)
            .unwrap()
            .declaration,
        "width: 12px;"
    );
}

#[test]
fn preserves_css_fallbacks_and_multiple_selector_targets() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"@utility paint-* { color: red; color: color(display-p3 1 0 0); }
@utility paint-* { &::before { content: "fallback"; } }"#,
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    let resolved = resolve_utility("paint-card", &theme).unwrap();

    assert!(
        resolved
            .declaration
            .contains_css("color: red; color: color(display-p3 1 0 0);")
            || resolved.additional.iter().any(|target| target
                .declaration
                .contains_css("color: red; color: color(display-p3 1 0 0);")),
        "{resolved:#?}"
    );
    // Targets with distinct selectors follow registration order, as Tailwind emits them.
    assert_eq!(resolved.selector, None);
    assert_eq!(
        resolved.declaration,
        "color: red; color: color(display-p3 1 0 0);"
    );
    assert_eq!(resolved.additional.len(), 1);
    assert_eq!(
        resolved.additional[0].selector.as_deref(),
        Some("&::before")
    );
    assert_eq!(resolved.additional[0].declaration, "content: \"fallback\";");
}

#[test]
fn color_alpha_family_emits_srgb_fallback_and_oklab_upgrade() {
    let resolved = resolve_utility("text-brand/50", &theme()).unwrap();
    assert_eq!(
        resolved.declaration,
        "color:color-mix(in srgb, red 50%, transparent);"
    );
    assert!(resolved.nested.iter().any(|rule| {
        rule.at_rule == NestedUtilityAtRule::Supports
            && rule.condition == "color:color-mix(in lab, red, red)"
            && rule.declaration == "color:color-mix(in oklab, var(--color-brand) 50%, transparent);"
    }));
    assert!(resolve_utility("text-brand/[length:12px]", &theme()).is_none());
}

#[test]
fn shadow_color_alpha_emits_hex_fallback_and_upgrade() {
    let unmodified = resolve_utility("shadow-red-500", &theme()).unwrap();
    assert_eq!(
        unmodified.declaration,
        "--tw-shadow-color:oklch(63.7% 0.237 25.331);"
    );
    assert!(unmodified.nested.iter().any(|rule| {
            rule.declaration.contains_css(
                "--tw-shadow-color:color-mix(in oklab, var(--color-red-500) var(--tw-shadow-alpha), transparent);",
            )
        }));

    let resolved = resolve_utility("shadow-red-500/50", &theme()).unwrap();
    assert!(resolved.declaration.contains_css(
        "--tw-shadow-color:color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);"
    ));
    assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab, red, red)"
                && rule.declaration.contains_css(
                    "--tw-shadow-color:color-mix(in oklab, color-mix(in oklab, var(--color-red-500) 50%, transparent) var(--tw-shadow-alpha), transparent);",
                )
        }));
}

#[test]
fn untyped_variables_select_the_earliest_registered_overload() {
    let theme = theme();
    for (candidate, expected) in [
        ("border-(--x)", "border-color:var(--x);"),
        ("border-[var(--x)]", "border-color:var(--x);"),
        ("border-[initial]", "border-color:initial;"),
        ("text-(--x)", "color:var(--x);"),
        ("text-[var(--x)]", "color:var(--x);"),
        ("stroke-(--x)", "stroke:var(--x);"),
        (
            "font-(--x)",
            "--tw-font-weight:var(--x);font-weight:var(--x);",
        ),
        ("font-[Inter]", "font-family:Inter;"),
        ("font-[500]", "--tw-font-weight:500;font-weight:500;"),
    ] {
        assert_css_eq!(
            resolve_utility(candidate, &theme).unwrap().declaration,
            expected
        );
    }
    let from = resolve_utility("from-(--x)", &theme).unwrap().declaration;
    assert!(from.contains_css("--tw-gradient-from:var(--x);"), "{from}");
    assert!(!from.contains("--tw-gradient-from-position:"), "{from}");
    for (candidate, shadow) in [
        ("shadow-(--x)", "--tw-shadow:var(--x);"),
        ("shadow-[var(--x)]", "--tw-shadow:var(--x);"),
        (
            "drop-shadow-(--x)",
            "--tw-drop-shadow-size:drop-shadow(var(--x));",
        ),
        ("inset-shadow-(--x)", "--tw-inset-shadow:inset var(--x);"),
        (
            "inset-shadow-[0_1px_red]",
            "--tw-inset-shadow:inset 0 1px var(--tw-inset-shadow-color, red);",
        ),
    ] {
        let declaration = resolve_utility(candidate, &theme).unwrap().declaration;
        assert!(
            declaration.contains_css(shadow),
            "{candidate}: {declaration}"
        );
        assert!(
            !declaration.contains("-color:var(--x)"),
            "{candidate}: {declaration}"
        );
    }
    assert!(
        resolve_utility("transition-(--x)", &theme)
            .unwrap()
            .declaration
            .contains_css(
                "transition-duration:var(--tw-duration, var(--default-transition-duration));"
            )
    );

    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"@utility probe-* { tab-size: --value([integer]); }
@utility probe-* { outline-color: --value([color]); }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    assert_css_eq!(
        resolve_utility("probe-(--x)", &theme).unwrap().declaration,
        "tab-size:var(--x);"
    );
}

#[test]
fn theme_functions_in_arbitrary_values_inline_token_values() {
    let theme = theme();
    for (candidate, expected) in [
        ("[--x:theme(--color-brand)]", "--x:red;"),
        ("[--x:theme(--color-missing,_blue)]", "--x:blue;"),
        ("bg-[theme(--color-brand)]", "background-color:red;"),
        ("w-[calc(theme(--breakpoint-sm)*2)]", "width:calc(40rem*2);"),
    ] {
        assert_css_eq!(
            resolve_utility(candidate, &theme).unwrap().declaration,
            expected
        );
    }
    assert!(resolve_utility("[--x:theme(--color-missing)]", &theme).is_none());
}
