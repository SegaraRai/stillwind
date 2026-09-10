use super::*;

#[test]
fn extracts_block_custom_variant() {
    let source = SourceFile::new(
        FileId(1),
        "fixture.css",
        "@custom-variant theme-dark { &:where([data-theme=dark] *) { @slot; } }",
    )
    .unwrap();
    let theme = parse_fixture_theme(&source).expect("theme");
    assert_eq!(theme.variants.len(), 1);
    assert_eq!(theme.variants[0].name, "theme-dark");
    assert!(
        theme.variants[0].body.contains("@slot"),
        "{:?}",
        theme.variants[0].body
    );
}

#[test]
fn separates_compiler_owned_utility_and_variant_directives_from_userland() {
    let css = "@utility user-* { color:red; }\
                   @stillwind-utility builtin-* { color:blue; }\
                   @variant user-state { &:hover { @slot; } }\
                   @stillwind-variant builtin-state { &:focus { @slot; } }";

    let user_utilities = parse_utility_definitions(css);
    let builtin_utilities = parse_builtin_utility_definitions(css);
    assert_eq!(user_utilities.len(), 1);
    assert_eq!(user_utilities[0].name, "user-*");
    assert_eq!(user_utilities[0].builtin_order, None);
    assert_eq!(builtin_utilities.len(), 1);
    assert_eq!(builtin_utilities[0].name, "builtin-*");
    assert_eq!(
        builtin_utilities[0].builtin_order,
        css.find("@stillwind-utility")
    );

    let user_variants = parse_variant_definitions(css);
    let builtin_variants = parse_builtin_variant_definitions(css);
    assert_eq!(user_variants.len(), 1);
    assert_eq!(user_variants[0].name, "user-state");
    assert_eq!(user_variants[0].builtin_order, None);
    assert_eq!(builtin_variants.len(), 1);
    assert_eq!(builtin_variants[0].name, "builtin-state");
    assert_eq!(builtin_variants[0].builtin_order, Some(0));
}

#[test]
fn shares_source_order_between_utilities_and_arbitrary_properties() {
    let css = "@stillwind-arbitrary-property margin;\
                   @stillwind-utility m-* { margin:--value([length]); }\
                   @stillwind-arbitrary-property --tw-*;\
                   @stillwind-arbitrary-property *;";
    let utility = parse_builtin_utility_definitions(css)
        .into_iter()
        .next()
        .expect("utility");
    let properties = parse_builtin_arbitrary_property_order(css);

    assert_eq!(properties.len(), 3);
    assert!(properties[0].builtin_order < utility.builtin_order.unwrap());
    assert!(utility.builtin_order.unwrap() < properties[1].builtin_order);
    assert_eq!(properties[1].pattern, "--tw-*");
    assert_eq!(properties[2].pattern, "*");
}

#[test]
fn parses_animation_options_and_namespace_type_metadata() {
    let css = "@stillwind-animation-option duration-* { animation-duration: --value(--duration-*); }\
                   @stillwind-animation-option-property transition-duration: animation-duration;\
                   @stillwind-theme-namespace --duration-* [time];\
                   @stillwind-theme-namespace --leading-* [number] [length];";
    let options = parse_builtin_animation_option_definitions(css);
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].name, "duration-*");
    assert_eq!(
        parse_animation_option_property_mappings(css).get("transition-duration"),
        Some(&"animation-duration".to_owned())
    );
    let namespaces = parse_theme_namespace_types(css);
    assert_eq!(namespaces[&ThemeNamespace::Duration], vec![ValueType::Time]);
    assert_eq!(
        namespaces[&ThemeNamespace::Leading],
        vec![ValueType::Number, ValueType::Length]
    );
}
