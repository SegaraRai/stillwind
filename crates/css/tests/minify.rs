//! Final CSS processing contracts, including input-map composition.

#![cfg(feature = "minify")]

use lean_string::LeanStr;
use parcel_sourcemap::{OriginalLocation, SourceMap};

use stillwind_css::{CssMinifyError, CssMinifyOptions, minify_css};

#[test]
fn implicit_scope_roots_are_preserved_or_reported_when_lowering_is_required() {
    let css = ".parent{@scope{.item{color:red}}}";
    assert_eq!(
        minify_css(css, &CssMinifyOptions::default()).unwrap().css,
        ".parent{@scope{& .item{color:red}}}"
    );
    let options = CssMinifyOptions {
        include: vec!["Nesting".into()],
        ..CssMinifyOptions::default()
    };
    assert!(
        minify_css(css, &options)
            .unwrap_err()
            .to_string()
            .contains("implicit @scope")
    );
}

#[test]
fn scope_nesting_keeps_ancestor_constraints_and_zero_specificity() {
    let options = CssMinifyOptions {
        include: vec!["Nesting".into()],
        ..CssMinifyOptions::default()
    };
    let result = minify_css(
        ".parent{@scope (.from) to (& .stop){color:red;.item{display:flex}}}",
        &options,
    )
    .unwrap();
    assert_eq!(
        result.css,
        "@scope(.parent .from) to (:where(:scope) .stop){:where(:scope){color:red}.item{display:flex}}"
    );
    assert_eq!(result.warnings, [] as [LeanStr; 0]);
}

#[test]
fn nested_scopes_keep_each_boundary() {
    let result = minify_css(
        "@scope (.outer){@scope (& .inner){color:red}}",
        &CssMinifyOptions {
            include: vec!["Nesting".into()],
            ..CssMinifyOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.css,
        "@scope(.outer){@scope(:scope .inner){:where(:scope){color:red}}}"
    );
}

#[test]
fn scoped_source_maps_keep_original_declaration_coordinates() {
    let css = "/* 日本語 */\n@scope (.card) {\n color:red;\n}";
    let options = CssMinifyOptions {
        source_map: true,
        ..CssMinifyOptions::default()
    };
    let output = minify_css(css, &options).unwrap();
    let json = output.source_map.unwrap();
    let mut map = SourceMap::from_json("", &json).unwrap();
    let column = u32::try_from(output.css.find(":where").unwrap()).unwrap();
    let original = map
        .find_closest_mapping(0, column)
        .unwrap()
        .original
        .unwrap();
    assert_eq!((original.original_line, original.original_column), (2, 1));
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["sourcesContent"][0], css);
}

#[test]
fn preserves_notice_text_during_minification() {
    let prefix = "/*! stillwind v0.0.0 */\n/*! 作者\n  license */\n";
    let css = format!("{prefix}.a {{ color: red; }}");
    let output = minify_css(&css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(output.css, format!("{prefix}.a{{color:red}}"));
}

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
fn removes_separator_whitespace_from_unparsed_values() {
    let css = ".a{--f:Arial, \"Segoe UI\" , sans-serif;--m:a * b / c;--c:calc(1 / * 2) calc(1 * = 2);font-family:var(--f,Arial, sans-serif);padding:calc(var(--s) * 4) calc(var(--s) / 2 - 1px) max(var(--s) * 2, 1px)}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(
        output.css,
        ".a{--f:Arial,\"Segoe UI\",sans-serif;--m:a * b / c;--c:calc(1/ *2) calc(1* = 2);font-family:var(--f,Arial,sans-serif);padding:calc(var(--s)*4) calc(var(--s)/2 - 1px) max(var(--s)*2,1px)}"
    );
}

#[test]
fn folds_numeric_calc_only_when_the_literal_is_equivalent() {
    let css = ".a{--p:calc(95 * 1%);--l:calc(2 * 3px);--z:calc(0 * 1px);--i:calc(2 * 2);--n:calc(50 * 1% * -1);--g:calc(90 * 1deg);--t:calc(2 * 100ms);--d:calc(10px / 4);--o:calc(100 / 100);--f:calc(98 / 100);--h:calc(1.5 * 1);--r:calc(1px / 3);--u:calc(10px / 2px);--v:calc(var(--a) * 2);opacity:if(style(--x:1):calc(50 * 1%);else:1)}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(
        output.css,
        ".a{--p:95%;--l:6px;--z:0px;--i:4;--n:-50%;--g:90deg;--t:200ms;--d:2.5px;--o:1;--f:calc(98/100);--h:calc(1.5*1);--r:calc(1px/3);--u:calc(10px/2px);--v:calc(var(--a)*2);opacity:if(style(--x:1):50%;else:1)}"
    );
}

#[test]
fn folds_fractions_only_for_properties_registered_as_numbers() {
    let css = "@property --n{syntax:\"<number>\";inherits:false;initial-value:1}@property --i{syntax:\"<integer> | <number>\";inherits:false;initial-value:1}@layer l{@property --m{syntax:\"<number>+\";inherits:false;initial-value:1}}.a{--n:calc(98 / 100);--i:calc(98 / 100);--m:calc(98 / 100);--u:calc(98 / 100)}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert!(
        output
            .css
            .ends_with(".a{--n:.98;--i:calc(98/100);--m:.98;--u:calc(98/100)}"),
        "{}",
        output.css
    );
}

#[test]
fn shortens_media_queries_without_changing_their_matches() {
    let css = "@media (hover:hover) and (prefers-reduced-motion:reduce){.a{color:red}}@media (pointer:fine){.b{color:red}}@media screen and (min-width:640px) and (hover:hover) and (max-width:1023.98px){.c{color:red}}@container x (width>=1px) and (width<9px){.d{color:red}}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(
        output.css,
        "@media (hover) and (prefers-reduced-motion){.a{color:red}}@media (pointer:fine){.b{color:red}}@media screen and (640px<=width<=1023.98px) and (hover){.c{color:red}}@container x (1px<=width<9px){.d{color:red}}"
    );
    let options = CssMinifyOptions {
        targets: Some([("safari".into(), 15 << 16)].into()),
        ..CssMinifyOptions::default()
    };
    assert_eq!(
        minify_css(
            "@media (min-width:640px) and (max-width:1023px){.c{color:red}}",
            &options
        )
        .unwrap()
        .css,
        "@media (min-width:640px) and (max-width:1023px){.c{color:red}}"
    );
}

#[test]
fn compacts_supports_declarations_without_rewriting_their_tokens() {
    let css =
        "@supports (color:color-mix(in lab, red , red)) and (font: 1px / 2 a , b){.a{color:red}}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(
        output.css,
        "@supports (color:color-mix(in lab,red,red)) and (font:1px / 2 a,b){.a{color:red}}"
    );
}

#[test]
fn removes_repeated_selectors_and_merges_equal_keyframes() {
    let css = ".a,.b,.a{color:red}@keyframes k{0%{opacity:0}50%{opacity:0}to{opacity:1}}@keyframes d{from{opacity:0}50%{opacity:0}0%{opacity:1}}";
    let output = minify_css(css, &CssMinifyOptions::default()).unwrap();
    assert_eq!(
        output.css,
        ".a,.b{color:red}@keyframes k{0%,50%{opacity:0}to{opacity:1}}@keyframes d{0%{opacity:0}50%{opacity:0}0%{opacity:1}}"
    );
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
