use super::*;

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
