use super::*;

#[test]
fn validates_csswg_property_grammars_with_lightningcss_or_exceptions() {
    for (name, valid, invalid) in [
        ("padding", "calc(1rem + 50%)", "red"),
        ("margin", "auto", "red"),
        ("border-width", "thin", "50%"),
        ("transition-duration", "150ms", "red"),
        ("aspect-ratio", "16 / 9", "red"),
        ("opacity", ".5", "red"),
        // This property was deliberately absent from the old hand-written
        // list. Its acceptance proves the domain comes from Lightning
        // CSS's generated property registry, not a fixture-derived list.
        ("text-decoration-line", "underline overline", "12px"),
        ("view-transition-name", "hero", "12px"),
        ("contain", "layout paint", "middle"),
        ("font-feature-settings", "\"liga\" 1", "liga"),
        ("object-position", "left 25%", "middle middle"),
        ("will-change", "transform, opacity", "1px"),
        ("zoom", "125%", "red"),
    ] {
        let property = CssProperty::from_name(name).unwrap();
        assert!(property.accepts(valid), "{name}: {valid}");
        assert!(!property.accepts(invalid), "{name}: {invalid}");
    }

    assert!(webref_property("padding").is_some());
    assert!(webref_property("grid-template-columns").is_some());
    assert!(webref_property("view-transition-name").is_some());
    assert!(webref_property("not-a-css-property").is_none());
    assert!(ValueDomain::from_hint("property:padding").is_none());
}

#[test]
fn generated_webref_snapshot_covers_every_internal_complex_value_domain() {
    for property in [
        "animation",
        "columns",
        "contain",
        "cursor",
        "flex",
        "font-feature-settings",
        "font-stretch",
        "grid-auto-columns",
        "grid-column",
        "grid-template-columns",
        "list-style-image",
        "list-style-type",
        "tab-size",
        "transition-property",
        "vertical-align",
        "will-change",
        "zoom",
    ] {
        let definition =
            webref_property(property).unwrap_or_else(|| panic!("Webref must define `{property}`"));
        assert!(!definition.syntax.is_empty(), "{property}");
        assert!(definition.href.starts_with("https://"), "{property}");
    }
}

#[test]
fn validates_anchor_positioning_with_explicit_property_exceptions() {
    let domain = |name| CssProperty::from_name(name).unwrap();

    assert!(domain("anchor-name").accepts("--card --tooltip"));
    assert!(!domain("anchor-name").accepts("card"));
    assert!(domain("position-anchor").accepts("--card"));
    assert!(domain("position-anchor").accepts("auto"));
    assert!(!domain("position-anchor").accepts("card"));
    assert!(domain("anchor-scope").accepts("all"));
    assert!(domain("anchor-scope").accepts("--dialog"));
    assert!(!domain("anchor-scope").accepts("dialog"));
    assert!(domain("position-area").accepts("top span-inline-end"));
    assert!(!domain("position-area").accepts("middle middle"));
    assert!(domain("position-try-fallbacks").accepts("--fallback, flip-block"));
    assert!(!domain("position-try-fallbacks").accepts("fallback"));
}

#[test]
fn preserves_authored_shadow_colour_when_applying_an_alpha_modifier() {
    assert_eq!(
            shadow_with_color_fallback(
                "inset 0 2px 4px rgb(0 0 0 / 0.05)",
                "--tw-inset-shadow-color",
                "50%",
            ),
            Some(
                "inset 0 2px 4px var(--tw-inset-shadow-color, oklab(from rgb(0 0 0 / 0.05) l a b / 50%))"
                    .to_owned()
            )
        );
}

#[test]
fn rewrites_each_authored_colour_in_a_shadow_list() {
    assert_eq!(
            shadow_with_color_override(
                "0 0 1px rgb(255 0 0 / 0.4), 0 0 3px rgb(0 0 255 / 0.2)",
                "--tw-shadow-color",
                None,
            ),
            Some(
                "0 0 1px var(--tw-shadow-color, rgb(255 0 0 / 0.4)), 0 0 3px var(--tw-shadow-color, rgb(0 0 255 / 0.2))"
                    .to_owned()
            )
        );
}

#[test]
fn expands_compact_utility_families_without_enumerating_values() {
    let utilities = parse_utility_definitions(
        "@utility-family value=--value(--spacing-*, --spacing(), [length]) { inset: inset; inset-x: inset-inline; }",
    );

    assert_eq!(
        utilities,
        vec![
            CustomUtility {
                name: "inset-*".to_owned(),
                body: "inset:--value(--spacing-*, --spacing(), [length]);".to_owned(),
                builtin_order: None,
            },
            CustomUtility {
                name: "inset-x-*".to_owned(),
                body: "inset-inline:--value(--spacing-*, --spacing(), [length]);".to_owned(),
                builtin_order: None,
            },
        ]
    );
}

#[test]
fn expands_bare_colour_modifier_families() {
    let utilities = parse_utility_definitions(
        "@utility border { border-color:color-mix(in oklab,currentcolor --alpha(--modifier()),transparent); } @utility outline { outline-color:color-mix(in oklab,currentcolor --alpha(--modifier()),transparent); }",
    );

    assert_eq!(utilities.len(), 2);
    assert_eq!(utilities[0].name, "border");
    assert!(utilities[0].body.contains("border-color:color-mix"));
    assert_eq!(utilities[1].name, "outline");
    assert!(utilities[1].body.contains("outline-color:color-mix"));
}

#[test]
fn expands_mask_stop_roots_from_edge_metadata() {
    let utilities = parse_utility_definitions(
        "@utility mask-x-from-* { --tw-mask-left-from-position:--value(--spacing-*); } @utility mask-x-from-* { --tw-mask-right-from-color:--value(--color-*); } @utility mask-x-from-* { --alpha(--modifier()); }",
    );

    assert_eq!(utilities.len(), 3);
    assert!(
        utilities
            .iter()
            .all(|utility| utility.name == "mask-x-from-*")
    );
    assert!(
        utilities[0]
            .body
            .contains("--tw-mask-left-from-position:--value(--spacing-*")
    );
    assert!(
        utilities[1]
            .body
            .contains("--tw-mask-right-from-color:--value(--color-*")
    );
    assert!(utilities[2].body.contains("--alpha(--modifier())"));
}
