use super::*;

#[test]
fn extracts_spanned_theme_tokens_after_lightningcss_validates_css() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; }",
    )
    .unwrap();

    let theme = parse_theme(&source).unwrap();

    assert_eq!(theme.tokens.len(), 2);
    assert_eq!(theme.tokens[0].name.value, "--color-accent");
    assert_eq!(source.slice(theme.tokens[1].value.span).unwrap(), "1rem");
    assert_eq!(theme.value("color", "accent"), Some("oklch(.6 .2 250)"));
    assert!(theme.token("spacing", "missing").is_none());
}

#[test]
fn matches_tailwind_theme_token_emission_modes() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        "@theme { --color-used: red; --color-unused: blue; }\n@theme static { --spacing-static: 1rem; }\n@theme inline { --color-alias: var(--brand); }",
    ).unwrap();
    let theme = parse_theme(&source).unwrap();
    let required = theme.tokens_required_by("color:var(--color-used);", false);
    assert!(
        required
            .iter()
            .any(|token| token.name.value == "--color-used")
    );
    assert!(
        required
            .iter()
            .any(|token| token.name.value == "--spacing-static")
    );
    assert!(
        !required
            .iter()
            .any(|token| token.name.value == "--color-unused")
    );
    assert!(
        !required
            .iter()
            .any(|token| token.name.value == "--color-alias")
    );
    assert_eq!(theme.value("color", "alias"), Some("var(--brand)"));
}

#[test]
fn breakpoint_lookup_values_do_not_retain_theme_custom_properties() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        "@theme { --breakpoint-foo: 60rem; --breakpoint-東京: 70rem; }",
    )
    .unwrap();
    let theme = parse_theme(&source).unwrap();
    let required = theme.tokens_required_by(
        "@media (width >= 60rem) {} @media (width >= 70rem) {}",
        false,
    );

    assert!(required.is_empty());
    let referenced = theme.tokens_required_by("width:var(--breakpoint-foo);", false);
    assert_eq!(referenced.len(), 1);
    assert_eq!(referenced[0].name.value, "--breakpoint-foo");
}

#[test]
fn extracts_css_first_stillwind_config_and_removes_it_from_custom_css() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        r#"@stillwind { prefix: "--app-"; lint { prefer-typed-relationships: warn; } }
.card { color: red; }"#,
    )
    .unwrap();

    let theme = parse_theme(&source).unwrap();

    assert_eq!(theme.config.prefix.as_deref(), Some("--app-"));
    assert!(theme.config.lint_enabled("prefer-typed-relationships"));
    assert!(!theme.custom_css().contains("@stillwind"));
    assert!(theme.custom_css().contains(".card"));
}

#[test]
fn applies_tailwind_defaults_and_namespace_resets() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        "@theme { --color-*: initial; --color-brand: oklch(.6 .2 250); }",
    )
    .unwrap();

    let theme = parse_fixture_theme(&source).unwrap();

    assert_eq!(theme.value("color", "brand"), Some("oklch(.6 .2 250)"));
    assert!(theme.token("color", "red-500").is_none());
    assert!(
        theme
            .active_tokens()
            .iter()
            .any(|token| token.name.value == "--spacing")
    );
}

#[test]
fn individual_imports_require_builtins_and_gate_default_theme_tokens() {
    let individual = parse_fixture_theme(
        &SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind/utilities.css\" layer(utilities);",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(individual.registry.utilities.is_empty());
    assert!(individual.scale(ThemeNamespace::Spacing).is_none());

    let with_builtins = parse_fixture_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ).unwrap())
        .unwrap();
    assert!(!with_builtins.registry.utilities.is_empty());
    assert_eq!(
        with_builtins
            .scale(ThemeNamespace::Spacing)
            .map(|token| token.value.value.as_str()),
        Some("0.25rem")
    );
    assert!(!with_builtins.custom_css().contains("@import"));

    let aggregate = parse_fixture_theme(
        &SourceFile::new(FileId(7), "stillwind.css", "@import \"stillwind\";").unwrap(),
    )
    .unwrap();
    assert!(!aggregate.registry.utilities.is_empty());
    assert!(!aggregate.tokens.is_empty());
    assert!(aggregate.custom_css.contains("box-sizing"));
    assert!(!aggregate.output_template.nodes.is_empty());

    let external_import = parse_theme(
        &SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"tailwindcss/utilities\" layer(utilities);",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        external_import
            .custom_css()
            .contains("@import \"tailwindcss/utilities\"")
    );
}

#[test]
fn consumes_framework_imports_but_preserves_other_css_imports() {
    let theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"./fonts.css\"; @import \"stillwind/builtins.css\"; .card { color: red; }",
        )
        .unwrap(),
    )
    .unwrap();

    assert!(theme.custom_css().contains("@import \"./fonts.css\";"));
    assert!(!theme.custom_css().contains("stillwind/builtins.css"));
    assert!(theme.custom_css().contains(".card"));
}

#[test]
fn retains_default_tokens_after_nested_theme_keyframes() {
    let theme = parse_fixture_theme(
        &SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@theme { --color-brand: rebeccapurple; }",
        )
        .unwrap(),
    )
    .unwrap();

    assert!(theme.token_in(ThemeNamespace::Blur, "sm").is_some());
    assert!(
        theme
            .token_in(ThemeNamespace::Perspective, "normal")
            .is_some()
    );
    assert!(theme.token_in(ThemeNamespace::Color, "brand").is_some());
}

#[test]
fn classifies_tailwind_theme_namespaces_without_losing_source_spans() {
    let source = SourceFile::new(
        FileId(7),
        "stillwind.css",
        "@theme { --spacing: .25rem; --color-accent: oklch(.6 .2 250); --vendor-value: 1; }",
    )
    .unwrap();

    let theme = parse_theme(&source).unwrap();
    let color = theme.token_in(ThemeNamespace::Color, "accent").unwrap();

    assert_eq!(
        color.kind,
        ThemeTokenKind::Token {
            namespace: ThemeNamespace::Color,
            name: "accent".to_owned(),
        }
    );
    assert_eq!(source.slice(color.name.span).unwrap(), "--color-accent");
    assert_eq!(
        theme.scale(ThemeNamespace::Spacing).unwrap().value.value,
        ".25rem"
    );
    assert_eq!(theme.tokens_in(ThemeNamespace::Color).len(), 1);
    assert_eq!(theme.tokens[2].kind, ThemeTokenKind::Custom);
}

#[test]
fn resolves_theme_reference_fallbacks_generically() {
    let source = SourceFile::new(
        FileId(7),
        "theme.css",
        r"@theme { --known: blue; --alias: --theme(--known, initial); }",
    )
    .unwrap();
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
    )
    .unwrap();

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
    )
    .unwrap();

    let theme = parse_theme(&source).unwrap();

    assert_eq!(theme.properties.len(), 1);
    assert_eq!(theme.properties[0].name, "--example-value");
    assert!(theme.authored_properties.contains("--authored-value"));
    assert!(!theme.custom_css().contains("@stillwind-property"));
    assert!(theme.custom_css().contains("@property --authored-value"));
}

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
