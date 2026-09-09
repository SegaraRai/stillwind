use stillwind_source::FileId;

use super::*;

#[test]
fn retains_each_utilitys_exact_relative_span() {
    let input = Spanned::new(
        "px-4  text-center\nβeta".to_owned(),
        Span::new(FileId(4), 12, 36),
    );

    let tokens = lex_utilities(&input);

    assert_eq!(
        tokens
            .iter()
            .map(|token| token.value.raw.as_str())
            .collect::<Vec<_>>(),
        ["px-4", "text-center", "βeta"],
    );
    assert_eq!(tokens[1].span, Span::new(FileId(4), 18, 29));
    assert_eq!(tokens[2].span, Span::new(FileId(4), 30, 35));
}

#[test]
fn parses_variants_modifiers_and_important_candidates() {
    let candidate = parse_candidate("hover:md:!bg-red-500/[.35]").unwrap();

    assert_eq!(candidate.raw, "hover:md:!bg-red-500/[.35]");
    assert!(candidate.important);
    assert!(!candidate.negative);
    assert_eq!(
        candidate.variants,
        vec![
            Variant {
                raw: "hover".to_owned(),
                kind: VariantKind::Named("hover".to_owned()),
            },
            Variant {
                raw: "md".to_owned(),
                kind: VariantKind::Named("md".to_owned()),
            },
        ]
    );
    assert_eq!(
        candidate.utility,
        Utility::Named {
            name: "bg-red-500".to_owned(),
            modifier: Some(Value::Arbitrary(".35".to_owned())),
        }
    );
}

#[test]
fn parses_negative_arbitrary_and_css_variable_candidates() {
    assert_eq!(
        parse_candidate("-translate-x-[calc(100%_-_1rem)]")
            .unwrap()
            .utility,
        Utility::Named {
            name: "translate-x-[calc(100%_-_1rem)]".to_owned(),
            modifier: None,
        }
    );
    assert_eq!(
        parse_candidate("w-(--sidebar)").unwrap().utility,
        Utility::Named {
            name: "w-(--sidebar)".to_owned(),
            modifier: None,
        }
    );
    assert_eq!(
        parse_candidate("[color:rebeccapurple]").unwrap().utility,
        Utility::ArbitraryProperty {
            property: "color".to_owned(),
            value: "rebeccapurple".to_owned(),
        }
    );
}

#[test]
fn parses_every_modifier_spelling_without_losing_the_utility_name() {
    let cases = [
        (
            "bg-red-500/50",
            Utility::Named {
                name: "bg-red-500".to_owned(),
                modifier: Some(Value::Named("50".to_owned())),
            },
        ),
        (
            "bg-red-500/[.35]",
            Utility::Named {
                name: "bg-red-500".to_owned(),
                modifier: Some(Value::Arbitrary(".35".to_owned())),
            },
        ),
        (
            "bg-red-500/(--brand-opacity)",
            Utility::Named {
                name: "bg-red-500".to_owned(),
                modifier: Some(Value::CssVariable("--brand-opacity".to_owned())),
            },
        ),
    ];

    for (raw, utility) in cases {
        assert_eq!(parse_candidate(raw).unwrap().utility, utility, "{raw}");
    }
}

#[test]
fn retains_nested_colons_in_arbitrary_variants_and_values() {
    let candidate =
        parse_candidate("[@supports(display:grid)]:[&:nth-child(3)]:content-['hello\\_world']")
            .unwrap();

    assert_eq!(
        candidate.variants,
        vec![
            Variant {
                raw: "[@supports(display:grid)]".to_owned(),
                kind: VariantKind::Arbitrary("@supports(display:grid)".to_owned()),
            },
            Variant {
                raw: "[&:nth-child(3)]".to_owned(),
                kind: VariantKind::Arbitrary("&:nth-child(3)".to_owned()),
            },
        ]
    );
    assert_eq!(
        candidate.utility,
        Utility::Named {
            name: "content-['hello\\_world']".to_owned(),
            modifier: None,
        }
    );
}

#[test]
fn parses_prefix_and_suffix_important_with_negative_utilities() {
    let cases = [("!flex", false), ("flex!", false), ("!-mt-4", true)];

    for (raw, negative) in cases {
        let candidate = parse_candidate(raw).unwrap();
        assert!(candidate.important, "{raw}");
        assert_eq!(candidate.negative, negative, "{raw}");
    }
}

#[test]
fn decodes_arbitrary_property_values_but_preserves_css_escapes_elsewhere() {
    assert_eq!(
        parse_candidate("[--sidebar:calc(100%_-_1rem)]")
            .unwrap()
            .utility,
        Utility::ArbitraryProperty {
            property: "--sidebar".to_owned(),
            value: "calc(100% - 1rem)".to_owned(),
        }
    );
    assert_eq!(
        parse_candidate("content-['hello\\_world']")
            .unwrap()
            .utility,
        Utility::Named {
            name: "content-['hello\\_world']".to_owned(),
            modifier: None,
        }
    );
}

#[test]
fn rejects_empty_dangling_and_unbalanced_candidates() {
    for raw in ["", ":", "hover:", "!", "bg-red-500/", "[", "hover:[&:focus"] {
        assert!(parse_candidate(raw).is_none(), "{raw}");
    }
}

#[test]
fn preserves_escaped_top_level_delimiters_and_important_markers() {
    let cases = ["icon\\:large", "bg-red\\/500", "flex\\!", "content-[\\]]"];

    for raw in cases {
        let candidate = parse_candidate(raw).unwrap();
        assert!(candidate.variants.is_empty(), "{raw}");
        assert!(!candidate.important, "{raw}");
        assert_eq!(
            candidate.utility,
            Utility::Named {
                name: raw.to_owned(),
                modifier: None,
            },
            "{raw}"
        );
    }
}
