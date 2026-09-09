use super::*;

#[test]
fn extracts_only_var_custom_property_dependencies() {
    let css =
        "--unused:red;--used:var(--source, var(--fallback));@property --ignored { syntax: '*' }";
    assert_eq!(
        referenced_custom_properties(css),
        BTreeSet::from(["--source".to_owned(), "--fallback".to_owned()])
    );
}

#[test]
fn validates_arbitrary_values_with_lightningcss_value_parsers() {
    for (value_type, valid, invalid) in [
        (ValueType::Integer, "calc(4 / 2)", "1.5"),
        (ValueType::Number, "calc(1 / 2)", "1px"),
        (ValueType::Percentage, "12.5%", "12px"),
        (ValueType::NumberPercentage, ".5", "1px"),
        (ValueType::Length, "calc(1rem + 2px)", "50%"),
        (ValueType::LengthPercentage, "calc(1rem + 50%)", "1s"),
        (ValueType::LineWidth, "thin", "50%"),
        (ValueType::ZIndex, "auto", "1.5"),
        (ValueType::Angle, "0.5turn", "1s"),
        (ValueType::AnglePercentage, "calc(90deg + 50%)", "1s"),
        (ValueType::Time, "150ms", "1px"),
        (ValueType::Ratio, "16 / 9", "1px"),
        (ValueType::Easing, "cubic-bezier(.2, .8, .2, 1)", "1px"),
        (ValueType::Color, "oklch(62% .2 280)", "12px"),
        (ValueType::Image, "linear-gradient(red, blue)", "red"),
        (
            ValueType::Url,
            "url(/hero.png)",
            "linear-gradient(red, blue)",
        ),
        (ValueType::Position, "right 1rem bottom 2px", "red"),
        (ValueType::BackgroundSize, "cover", "red"),
        (ValueType::Shadow, "0 1px 2px #000", "1rem"),
        (ValueType::Content, "'stillwind'", "stillwind"),
        (
            ValueType::LinearGradientArguments,
            "45deg, red, blue",
            "red / blue",
        ),
        (
            ValueType::RadialGradientArguments,
            "at 20% 30%, red, blue",
            "red / blue",
        ),
        (
            ValueType::ConicGradientArguments,
            "from 45deg, red, blue",
            "red / blue",
        ),
        (ValueType::SkewArguments, "15deg", "red"),
        (ValueType::SkewXArguments, "15deg", "red"),
        (ValueType::SkewYArguments, "15deg", "red"),
        (ValueType::Animation, "spin 1s linear infinite", "1px"),
        (ValueType::FontFamily, "ui-sans-serif, sans-serif", "12px"),
        (ValueType::FontSize, "1rem", "red"),
        (ValueType::FontWeight, "650", "red"),
        (ValueType::VerticalAlign, "middle", "red"),
        (ValueType::GridTrackSize, "minmax(0, 1fr)", "red"),
        (ValueType::GridLine, "span 2", "1px"),
        (ValueType::Columns, "12rem 3", "red"),
        (ValueType::Contain, "layout paint", "red"),
        (ValueType::Cursor, "pointer", "red"),
        (ValueType::FontFeatureSettings, "\"liga\" 1", "liga"),
        (ValueType::FontStretch, "125%", "red"),
        (ValueType::GridTrackList, "repeat(3, minmax(0, 1fr))", "red"),
        (ValueType::Flex, "1 1 0%", "red"),
        (ValueType::ListStyleType, "disc", "1px"),
        (ValueType::ListStyleImage, "url(icon.svg)", "red"),
        (ValueType::TabSize, "4", "red"),
        (ValueType::TransitionProperty, "transform", "1px"),
        (ValueType::WillChange, "transform", "1px"),
        (ValueType::Zoom, "125%", "red"),
    ] {
        assert!(value_type.accepts(valid), "{value_type:?}: {valid}");
        assert!(!value_type.accepts(invalid), "{value_type:?}: {invalid}");
    }
    assert_eq!(ValueType::from_hint("length"), Some(ValueType::Length));
    assert_eq!(
        ValueType::from_hint("length-percentage"),
        Some(ValueType::LengthPercentage)
    );
    assert_eq!(ValueType::from_hint("unknown"), None);
    assert!(ValueType::Integer.accepts("initial"));
    assert!(ValueType::Length.accepts("calc(var(--gutter) * 2)"));
    assert!(!ValueType::Length.accepts("var(gutter)"));
    assert!(ValueType::LengthPercentage.accepts_hint(ValueType::Percentage));
    assert!(ValueType::LengthPercentage.accepts_hint(ValueType::Length));
    assert!(!ValueType::Length.accepts_hint(ValueType::Percentage));
    assert!(ValueType::ZIndex.accepts_hint(ValueType::Integer));
    assert!(!ValueType::ZIndex.accepts_hint(ValueType::Number));
    assert!(ValueType::LineWidth.accepts_hint(ValueType::Length));
    assert!(ValueType::NumberPercentage.accepts_hint(ValueType::Percentage));
}

#[test]
fn extracts_block_custom_variant() {
    let source = SourceFile::new(
        FileId(1),
        "fixture.css",
        "@custom-variant theme-dark { &:where([data-theme=dark] *) { @slot; } }",
    );
    let theme = parse_tailwind_theme(&source).expect("theme");
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

#[test]
fn parses_builtin_variant_sort_options() {
    let variants = parse_builtin_variant_definitions(
        "@stillwind-variant max-* (sort: --breakpoint-*, direction: desc) { @slot; }\
             @stillwind-variant min-* (group: responsive-min, sort: --breakpoint-*, direction: asc, bare: true) { @slot; }",
    );

    assert_eq!(variants.len(), 2);
    assert_eq!(
        variants[0].sort.as_ref().unwrap().key.as_deref(),
        Some("--breakpoint-*")
    );
    assert_eq!(
        variants[0].sort.as_ref().unwrap().direction,
        Some(BuiltinVariantSortDirection::Descending)
    );
    assert_eq!(
        variants[1].sort.as_ref().unwrap().group.as_deref(),
        Some("responsive-min")
    );
    assert!(variants[1].sort.as_ref().unwrap().bare);
}

#[test]
fn parses_typed_variant_values_modifiers_and_literal_asterisks() {
    let variants = parse_builtin_variant_definitions(
        r"@stillwind-variant \* { :is(& > *) { @slot; } }
               @stillwind-variant group-* (value: [variant], modifier: [class-identifier]) { @slot; }
               @stillwind-variant supports-* (value: [supports-condition]) { @slot; }",
    );

    assert_eq!(variants.len(), 3);
    assert_eq!(variants[0].name, "*");
    assert!(!variants[0].wildcard);
    assert!(variants[1].wildcard);
    assert_eq!(
        variants[1].value_type,
        Some(BuiltinVariantValueType::Variant)
    );
    assert_eq!(
        variants[1].modifier_type,
        Some(BuiltinVariantModifierType::ClassIdentifier)
    );
    assert_eq!(
        variants[2].value_type,
        Some(BuiltinVariantValueType::SupportsCondition)
    );
}

#[test]
fn validates_nth_and_supports_conditions_through_lightning_css() {
    assert!(BuiltinVariantValueType::Nth.accepts_css("3n + 1"));
    assert!(BuiltinVariantValueType::Nth.accepts_css("odd"));
    assert!(!BuiltinVariantValueType::Nth.accepts_css("not-an-nth"));
    assert!(BuiltinVariantValueType::SupportsCondition.accepts_css("display: grid"));
}

#[test]
fn serializes_class_identifier_modifiers_for_css_selectors() {
    let kind = BuiltinVariantModifierType::ClassIdentifier;
    assert_eq!(kind.serialize("menu").as_deref(), Some("menu"));
    assert_eq!(
        kind.serialize(r"menu\:open").as_deref(),
        Some(r"menu\:open")
    );
    assert_eq!(kind.serialize("not an identifier"), None);
}
