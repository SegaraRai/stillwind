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

    let theme = parse_tailwind_theme(&source).unwrap();

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
    let individual = parse_tailwind_theme(
        &SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind/utilities.css\" layer(utilities);",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!individual.imports.builtins);
    assert!(individual.scale(ThemeNamespace::Spacing).is_none());

    let with_builtins = parse_tailwind_theme(&SourceFile::new(
            FileId(7),
            "stillwind.css",
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ).unwrap())
        .unwrap();
    assert!(with_builtins.imports.builtins);
    assert_eq!(
        with_builtins
            .scale(ThemeNamespace::Spacing)
            .map(|token| token.value.value.as_str()),
        Some("0.25rem")
    );
    assert!(!with_builtins.custom_css().contains("@import"));

    let aggregate = parse_tailwind_theme(
        &SourceFile::new(FileId(7), "stillwind.css", "@import \"stillwind\";").unwrap(),
    )
    .unwrap();
    assert!(aggregate.imports.builtins);
    assert!(aggregate.imports.theme_layer.is_some());
    assert!(aggregate.imports.base_layer.is_some());
    assert!(aggregate.imports.utilities_layer.is_some());

    let external_import = parse_tailwind_theme(
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
    let theme = parse_tailwind_theme(
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
    let theme = parse_tailwind_theme(
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
