use super::*;

#[test]
fn renders_effective_theme_custom_properties_with_tailwind_reset_semantics() {
    let fixture = include_str!("../../../../fixtures/tailwind-v4/cases/theme.fixture.md");
    let section = fixture
        .split("## `")
        .find(|section| section.starts_with("theme.namespace-reset-and-negative-token`"))
        .expect("theme reset fixture must exist");
    let input_css = fixture_code_block(section, "Input CSS", "css").unwrap();
    let component = SourceFile::new(
        FileId(1),
        "src/Fixture.tsx",
        "import { style } from \"stillwind/react\"; const props = style(\"bg-brand p-4\");",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &stillwind_js_oxc::OxcSourceAnalyzer,
        &component,
        &crate::css_fixture::source(
            FileId(2),
            "stillwind.css",
            stillwind_fixture_css(&input_css),
        )
        .unwrap(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("--spacing:0.5rem;"));
    assert!(result.css.contains("--color-brand:oklch(62% 0.2 280);"));
    assert!(!result.css.contains("--color-red-500:"));
}

#[test]
fn shadow_utilities_follow_multiple_theme_overrides_and_namespace_removal() {
    let fixture =
        include_str!("../../../../fixtures/tailwind-v4/cases/theme-shadow-tokens.fixture.md");
    let configured_cases = [
        (
            "theme-shadow-tokens.custom-default-size-overrides",
            ["shadow-lg", "inset-shadow-sm", "drop-shadow-sm"],
        ),
        (
            "theme-shadow-tokens.custom-alternate-size-overrides",
            ["shadow-xs", "inset-shadow-xs", "drop-shadow-xs"],
        ),
    ];

    for (case_name, candidates) in configured_cases {
        let section = fixture_case_named(fixture, case_name);
        let input_css = fixture_code_block(section, "Input CSS", "css").unwrap();
        let expected_css = fixture_code_block(section, "Expected CSS", "css").unwrap();
        for candidate in candidates {
            let component = SourceFile::new(
                FileId(1),
                "src/Fixture.tsx",
                format!(
                    "import {{ style }} from \"stillwind/react\"; const props = style({candidate:?});"
                ),
            ).unwrap();
            let result = Compiler::compile_source(
                &stillwind_js_oxc::OxcSourceAnalyzer,
                &component,
                &crate::css_fixture::source(
                    FileId(2),
                    "stillwind.css",
                    stillwind_fixture_css(&input_css),
                )
                .unwrap(),
            );
            assert!(result.diagnostics.is_empty(), "{case_name}: {candidate}");
            let class_name = &result.classes[0].class_name;
            let selector = tailwind_class_selector(candidate);
            let actual = css_rule(&result.css, class_name).replace(class_name, &selector);
            let expected = css_rule(&expected_css, &selector);
            assert_eq!(
                minify_css(&actual),
                minify_css(&expected),
                "{case_name}: {candidate}"
            );
        }
    }

    let removed = fixture_case_named(fixture, "theme-shadow-tokens.namespace-removal");
    let input_css = fixture_code_block(removed, "Input CSS", "css").unwrap();
    let candidates = fixture_code_block(removed, "Candidates", "text").unwrap();
    let result = Compiler::compile_source(
        &stillwind_js_oxc::OxcSourceAnalyzer,
        &SourceFile::new(
            FileId(3),
            "src/Fixture.tsx",
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style({candidates:?});"
            ),
        )
        .unwrap(),
        &crate::css_fixture::source(
            FileId(4),
            "stillwind.css",
            stillwind_fixture_css(&input_css),
        )
        .unwrap(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.classes.is_empty(), "{}", result.css);
    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|finding| finding.rule == "unknown-utility")
            .count(),
        6
    );
}

#[test]
fn framework_imports_gate_baseline_layers() {
    let component = SourceFile::new(
        FileId(1),
        "src/Fixture.tsx",
        "import { style } from \"stillwind/react\"; const props = style(\"p-4\");",
    )
    .unwrap();
    let compiler = |css| {
        Compiler::compile_source(
            &stillwind_js_oxc::OxcSourceAnalyzer,
            &component,
            &crate::css_fixture::source(FileId(2), "stillwind.css", css).unwrap(),
        )
    };

    let individual_without_builtins = compiler(
        "@import \"stillwind/theme.css\" layer(theme);\n@import \"stillwind/utilities.css\" layer(utilities);",
    );
    assert!(
        individual_without_builtins
            .lint_findings
            .iter()
            .any(|finding| finding.rule == "unknown-utility")
    );

    let utilities_only = compiler(
        "@import \"stillwind/theme.css\" layer(theme);\n@import \"stillwind/utilities.css\" layer(utilities);\n@import \"stillwind/builtins.css\";",
    );
    assert!(utilities_only.diagnostics.is_empty());
    assert!(!utilities_only.css.contains("box-sizing:border-box"));
    assert!(!utilities_only.css.contains("@property --tw-"));
    assert!(utilities_only.css.contains("@layer theme{:root,:host{"));
    assert!(utilities_only.css.contains("@layer utilities{"));

    let full = compiler("@import \"stillwind\";");
    assert!(full.diagnostics.is_empty());
    assert!(full.css.contains("box-sizing: border-box"));
    assert!(!full.css.contains("@property --tw-"));

    let tailwind_individual = compiler(
        "@import \"stillwind/theme.css\" layer(theme);\n@import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
    );
    assert!(tailwind_individual.diagnostics.is_empty());
    assert!(tailwind_individual.css.contains("@layer utilities{"));

    let content = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer,
        &SourceFile::new(
            FileId(3),
            "src/Content.tsx",
            "import { style } from \"stillwind/react\"; const props = style(\"bg-linear-to-r\");",
        ).unwrap(),
        &crate::css_fixture::source(
            FileId(4),
            "stillwind.css",
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ).unwrap(),
    );
    assert!(content.diagnostics.is_empty());
    assert!(!content.css.contains("@property --tw-gradient-position"));
}
