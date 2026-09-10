use super::*;

#[test]
fn resolves_user_utilities_with_the_same_value_domains_as_built_ins() {
    let source = SourceFile::new(
        FileId(1),
        "theme.css",
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

    assert_eq!(
        resolve_utility("tone-brand", &theme).unwrap().declaration,
        "color: var(--color-brand);"
    );
    assert_eq!(
        resolve_utility("rail-4", &theme).unwrap().declaration,
        "inline-size: calc(var(--spacing) * 4);"
    );
    assert_eq!(
        resolve_utility("-rail-4", &theme).unwrap().declaration,
        "inline-size: calc(var(--spacing) * -4);"
    );
    assert_eq!(
        resolve_utility("offset-auto", &theme).unwrap().declaration,
        "inset: auto;"
    );
    assert!(resolve_utility("-offset-auto", &theme).is_none());
    assert_eq!(
        resolve_utility("-offset-full", &theme).unwrap().declaration,
        "inset: -100%;"
    );
    assert_eq!(
        resolve_utility("mask-[length:12px]", &theme)
            .unwrap()
            .declaration,
        "mask-size: 12px;"
    );
    assert_eq!(
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
        "theme.css",
        r"@utility token-* { width: --value([length]); }
@utility token-* { color: --value([color]); }",
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
fn preserves_css_fallbacks_and_multiple_selector_targets() {
    let source = SourceFile::new(
        FileId(1),
        "theme.css",
        r#"@utility paint-* { color: red; color: color(display-p3 1 0 0); }
@utility paint-* { &::before { content: "fallback"; } }"#,
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).unwrap();
    let resolved = resolve_utility("paint-card", &theme).unwrap();

    assert!(
        resolved
            .declaration
            .contains("color: red; color: color(display-p3 1 0 0);")
            || resolved.additional.iter().any(|target| target
                .declaration
                .contains("color: red; color: color(display-p3 1 0 0);")),
        "{resolved:#?}"
    );
    assert_eq!(resolved.additional.len(), 1);
    assert_eq!(resolved.selector.as_deref(), Some("&::before"));
    assert_eq!(resolved.declaration, "content: \"fallback\";");
    assert_eq!(
        resolved.additional[0].declaration,
        "color: red; color: color(display-p3 1 0 0);"
    );
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
            rule.declaration.contains(
                "--tw-shadow-color:color-mix(in oklab, var(--color-red-500) var(--tw-shadow-alpha), transparent);",
            )
        }));

    let resolved = resolve_utility("shadow-red-500/50", &theme()).unwrap();
    assert!(resolved.declaration.contains(
        "--tw-shadow-color:color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);"
    ));
    assert!(resolved.nested.iter().any(|rule| {
            rule.at_rule == NestedUtilityAtRule::Supports
                && rule.condition == "color:color-mix(in lab, red, red)"
                && rule.declaration.contains(
                    "--tw-shadow-color:color-mix(in oklab, color-mix(in oklab, var(--color-red-500) 50%, transparent) var(--tw-shadow-alpha), transparent);",
                )
        }));
}
