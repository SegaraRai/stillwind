//! Final CSS processing contracts, including input-map composition.

#![cfg(feature = "minify")]

use parcel_sourcemap::{OriginalLocation, SourceMap};

use stillwind_css::{CssMinifyError, CssMinifyOptions, minify_css};

#[test]
fn combines_equal_queries_and_retains_conflicting_rule_barriers() {
    let css = ".a{display:grid}@supports (x:revert-rule) and (--a:){.a{display:var(--d,revert-rule)}}@supports (x:revert-rule) and (--a:){.a{opacity:var(--o,revert-rule)}}@supports (x:revert-rule) and (--b:){.a{display:var(--e,revert-rule)}}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(output.css.matches("@supports").count(), 2);
    assert!(output.css.starts_with(".a{display:grid}"));
    assert!(output.css.contains("(--a:)"));
    assert!(output.css.contains("(--b:)"));
    assert!(output.source_map.is_none());
    assert_eq!(output.warnings, [] as [String; 0]);
}

#[test]
fn targets_features_and_unused_symbols_are_forwarded() {
    let css = ".a{color:red;&:hover{color:blue}}.unused{height:1px}";
    let mut options = CssMinifyOptions {
        include: vec!["Nesting".into()],
        unused_symbols: vec!["unused".into()],
        ..CssMinifyOptions::default()
    };
    let output = minify_css(css, &options).unwrap();
    assert!(!output.css.contains('&'));
    assert!(!output.css.contains("unused"));
    options.include.clear();
    options.targets = Some([("chrome".into(), 100 << 16)].into());
    assert!(!minify_css(css, &options).unwrap().css.contains('&'));
    options.exclude.push("Nesting".into());
    assert!(minify_css(css, &options).unwrap().css.contains('&'));
}

#[test]
fn recovery_returns_warnings_while_strict_parsing_fails() {
    let css = ".a{color red;} .b{color:red}";
    assert!(matches!(
        minify_css(css, &CssMinifyOptions::default()),
        Err(CssMinifyError::Parse(_))
    ));
    let output = minify_css(
        css,
        &CssMinifyOptions {
            error_recovery: true,
            ..CssMinifyOptions::default()
        },
    )
    .unwrap();
    assert_ne!(output.warnings, [] as [String; 0]);
    assert!(output.css.contains(".b{color:red}"));
}

#[test]
fn generates_and_composes_maps_to_multiple_original_sources() {
    let css = ".a { color: red }\n.b { width: 1px }";
    let mut options = CssMinifyOptions {
        filename: "generated.css".into(),
        source_map: true,
        ..CssMinifyOptions::default()
    };
    let output = minify_css(css, &options).unwrap();
    let mut map = SourceMap::from_json("", output.source_map.as_deref().unwrap()).unwrap();
    let position = u32::try_from(output.css.find(".b").unwrap()).unwrap();
    let original = map
        .find_closest_mapping(0, position)
        .unwrap()
        .original
        .unwrap();
    assert_eq!(map.get_source(original.source).unwrap(), "generated.css");
    assert_eq!((original.original_line, original.original_column), (1, 0));

    let mut input = SourceMap::new("");
    let first = input.add_source("first.tsx");
    let second = input.add_source("second.css");
    input
        .set_source_content(first as usize, "first source")
        .unwrap();
    input
        .set_source_content(second as usize, "second source")
        .unwrap();
    input.add_mapping(0, 0, Some(OriginalLocation::new(10, 2, first, None)));
    input.add_mapping(1, 0, Some(OriginalLocation::new(20, 4, second, None)));
    options.input_source_map = Some((input.to_json(Some("../src/")).unwrap()).into());
    let output = minify_css(css, &options).unwrap();
    let json = output.source_map.unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["sourceRoot"], "../src/");
    let mut map = SourceMap::from_json("", &json).unwrap();
    for (column, source, line, original_column) in
        [(0, "first.tsx", 10, 2), (position, "second.css", 20, 4)]
    {
        let original = map
            .find_closest_mapping(0, column)
            .unwrap()
            .original
            .unwrap();
        assert_eq!(map.get_source(original.source).unwrap(), source);
        assert_eq!(
            (original.original_line, original.original_column),
            (line, original_column)
        );
    }
    options.include_sources_content = false;
    let json: serde_json::Value =
        serde_json::from_str(&minify_css(css, &options).unwrap().source_map.unwrap()).unwrap();
    assert!(json.get("sourcesContent").is_none());
}

#[test]
fn validates_options_and_maps_without_following_embedded_urls() {
    for options in [
        CssMinifyOptions {
            include: vec!["unknown".into()],
            ..CssMinifyOptions::default()
        },
        CssMinifyOptions {
            include: vec!["Selectors".into()],
            exclude: vec!["Nesting".into()],
            ..CssMinifyOptions::default()
        },
        CssMinifyOptions {
            targets: Some([("unknown".into(), 1)].into()),
            ..CssMinifyOptions::default()
        },
        CssMinifyOptions {
            input_source_map: Some("{}".into()),
            ..CssMinifyOptions::default()
        },
    ] {
        assert!(matches!(
            minify_css("", &options),
            Err(CssMinifyError::Options(_))
        ));
    }
    for map in ["invalid", r#"{"version":3,"sections":[]}"#] {
        assert!(matches!(
            minify_css(
                "",
                &CssMinifyOptions {
                    source_map: true,
                    input_source_map: Some(map.into()),
                    ..CssMinifyOptions::default()
                }
            ),
            Err(CssMinifyError::SourceMap(_))
        ));
    }
    let output = minify_css(
        ".a{color:red}/*# sourceMappingURL=data:application/json;base64,e30= */",
        &CssMinifyOptions {
            source_map: true,
            ..CssMinifyOptions::default()
        },
    )
    .unwrap();
    assert!(!output.css.contains("sourceMappingURL"));
    assert!(output.source_map.unwrap().contains("input.css"));
}
