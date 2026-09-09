use super::*;

#[test]
fn merges_declaration_and_function_format_metadata() {
    let formats = parse_declaration_formats(
        "@stillwind-format transform var-fallback; @stillwind-format transform compose-space;",
    );
    assert_eq!(
        formats.get("transform").map(String::as_str),
        Some("var-fallback compose-space")
    );
    let functions =
        parse_function_formats("@stillwind-format-functions color-mix linear-gradient;");
    assert!(functions.contains("color-mix"));
    assert!(functions.contains("linear-gradient"));
    assert_eq!(
        parse_omit_token_prefixes("@stillwind-omit-token-prefix --shadow- --tw-shadow:;"),
        vec![("--shadow-".to_owned(), "--tw-shadow:".to_owned())]
    );
}

#[test]
fn resolves_theme_reference_fallbacks_generically() {
    let source = SourceFile::new(
        FileId(7),
        "theme.css",
        r"@theme { --known: blue; --alias: --theme(--known, initial); }",
    );
    let theme = parse_theme(&source).unwrap();

    assert_eq!(
        theme.resolve_theme_references("--theme(--known, initial)"),
        Some("var(--known)".to_owned())
    );
    assert_eq!(
        theme.resolve_theme_references("calc(1px + --theme(--missing, 2px))"),
        Some("calc(1px + 2px)".to_owned())
    );
    assert_eq!(
        theme.resolve_theme_references("--theme(--missing, initial)"),
        None
    );
    assert_eq!(
        theme.resolve_theme_references("\"--theme(--missing, initial)\""),
        Some("\"--theme(--missing, initial)\"".to_owned())
    );
}

#[test]
fn parses_block_variants_and_omits_them_from_authored_output() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        "@variant pressed { &:active { @slot; } }\n.card { color: red; }",
    );

    let theme = parse_theme(&source).unwrap();

    assert_eq!(
        theme.variant("pressed").unwrap().body,
        "&:active { @slot; }"
    );
    assert!(theme.custom_css().contains(".card"));
    assert!(!theme.custom_css().contains("@variant"));
}

#[test]
fn separates_compiler_property_definitions_from_authored_property_rules() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        r#"
                @stillwind-property --example-value {
                  syntax: "<number>";
                  inherits: false;
                  initial-value: 0;
                }
                @property --authored-value {
                  syntax: "<color>";
                  inherits: false;
                  initial-value: red;
                }
                .example { color: var(--authored-value); }
            "#,
    );

    let theme = parse_theme(&source).unwrap();

    assert_eq!(theme.properties.len(), 1);
    assert_eq!(theme.properties[0].name, "--example-value");
    assert!(theme.authored_properties.contains("--authored-value"));
    assert!(!theme.custom_css().contains("@stillwind-property"));
    assert!(theme.custom_css().contains("@property --authored-value"));
}

#[test]
fn parses_property_activation_and_dependency_metadata() {
    let definitions = parse_property_definitions(
        r#"
                @stillwind-property --example {
                  syntax: "*";
                  inherits: false;
                  activation: referenced;
                  depends-on: --dependency-a --dependency-b;
                  group: example-group shared-group;
                  activate-on: text-shadow box-shadow;
                }
            "#,
    );

    assert_eq!(definitions.len(), 1);
    assert_eq!(
        definitions[0].activation,
        CustomPropertyActivation::Referenced
    );
    assert_eq!(
        definitions[0].dependencies,
        ["--dependency-a", "--dependency-b"]
    );
    assert_eq!(definitions[0].groups, ["example-group", "shared-group"]);
    assert_eq!(
        definitions[0].activation_properties,
        ["text-shadow", "box-shadow"]
    );
    assert!(!definitions[0].body.contains("activation"));
    assert!(!definitions[0].body.contains("depends-on"));
    assert!(!definitions[0].body.contains("group"));
    assert!(!definitions[0].body.contains("activate-on"));
}

#[test]
fn classifies_custom_property_usage_from_the_css_ast() {
    let usage = custom_property_usage(
            "@layer utilities { .example { --declared: var(--referenced, var(--fallback)); color: var(--color); scroll-snap-type: x var(--snap); transform: var(--transform,) } }",
        )
        .unwrap();

    assert_eq!(usage.declared, BTreeSet::from(["--declared".to_owned()]));
    assert!(usage.declaration_properties.contains("color"));
    assert!(usage.declaration_properties.contains("scroll-snap-type"));
    assert_eq!(
        usage.referenced,
        BTreeSet::from([
            "--color".to_owned(),
            "--fallback".to_owned(),
            "--referenced".to_owned(),
            "--snap".to_owned(),
            "--transform".to_owned(),
        ])
    );
}

#[test]
fn folds_calc_expressions_through_a_numeric_syntax_tree() {
    assert_eq!(
        fold_calc_expressions(
            "width:calc(2 * 1px);scale:calc(125 * 1%);left:calc(var(--spacing) * 4);right:calc((2px + 3px) * -1);"
        ),
        "width:2px;scale:125%;left:calc(var(--spacing) * 4);right:-5px;"
    );
}

#[test]
fn folds_trailing_numeric_factors_without_string_rewrites() {
    assert_eq!(
        fold_calc_expressions(
            "left:calc(var(--spacing) * 4 * -1);right:calc(var(--spacing) * -1 * 4);"
        ),
        "left:calc(var(--spacing) * -4);right:calc(var(--spacing) * -4);"
    );
}
