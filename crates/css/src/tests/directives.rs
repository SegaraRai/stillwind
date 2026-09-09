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
