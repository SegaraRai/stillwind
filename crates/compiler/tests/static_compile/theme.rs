//! Theme emission and token gating.
//!
//! Pins Tailwind's reset semantics for effective custom properties, shadow-utility
//! behavior across multiple `@theme` overrides and namespace removal, and that
//! framework imports gate the baseline layers.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::{CssAssertions as _, case_named, code_block, css_source, fixture_css};

use super::*;

#[test]
fn malformed_theme_reference_is_a_diagnostic_instead_of_an_omitted_token() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from 'stillwind/react'; const props = style('bg-bad');",
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@import 'stillwind'; @theme { --color-bad: --theme(x); }",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert!(
        result.diagnostics.iter().any(|error| matches!(
            error.diagnostic,
            CompileDiagnostic::InvalidThemeReference { offset: 0, .. }
        )),
        "{result:#?}"
    );
}

#[test]
fn renders_effective_theme_custom_properties_with_tailwind_reset_semantics() {
    let fixture = include_str!("../../../../fixtures/tailwind-v4/cases/theme.fixture.md");
    let section = fixture
        .split("## `")
        .find(|section| section.starts_with("theme.namespace-reset-and-negative-token`"))
        .expect("theme reset fixture must exist");
    let input_css = code_block(section, "Input CSS", "css").unwrap();
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from \"stillwind/react\"; const props = style(\"bg-brand p-4\");",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &css_source(FileId(2), fixture_css(&input_css)).unwrap(),
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains_css("--spacing:0.5rem;"));
    assert!(result.css.contains_css("--color-brand:oklch(62% 0.2 280);"));
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
        let section = case_named(fixture, case_name);
        let input_css = code_block(section, "Input CSS", "css").unwrap();
        let expected_css = code_block(section, "Expected CSS", "css").unwrap();
        for candidate in candidates {
            let component = SourceFile::new(
                FileId(1), None, None,
                format!(
                    "import {{ style }} from \"stillwind/react\"; const props = style({candidate:?});"
                ),
            ).unwrap();
            let result = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &component,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &css_source(FileId(2), fixture_css(&input_css)).unwrap(),
                &CssSourceName::from("stillwind.css"),
                &CompilerOptions::default(),
            );
            assert!(result.diagnostics.is_empty(), "{case_name}: {candidate}");
            let class_name = &result.classes[0].class_name;
            let selector = tailwind_class_selector(candidate);
            let actual =
                css_rule(&result.css, class_name).replace((class_name).as_str(), &selector);
            let expected = css_rule(&expected_css, &selector);
            assert_eq!(
                minify_css(&actual),
                minify_css(&expected),
                "{case_name}: {candidate}"
            );
        }
    }

    let removed = case_named(fixture, "theme-shadow-tokens.namespace-removal");
    let input_css = code_block(removed, "Input CSS", "css").unwrap();
    let candidates = code_block(removed, "Candidates", "text").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &SourceFile::new(
            FileId(3),
            None,
            None,
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style({candidates:?});"
            ),
        )
        .unwrap(),
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &css_source(FileId(4), fixture_css(&input_css)).unwrap(),
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.classes.is_empty(), "{}", result.css);
    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|finding| matches!(finding.diagnostic, LintDiagnostic::UnknownUtility { .. }))
            .count(),
        6
    );
}

#[test]
fn framework_imports_gate_baseline_layers() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from \"stillwind/react\"; const props = style(\"p-4\");",
    )
    .unwrap();
    let compiler = |css| {
        Compiler::compile_source(
            &OxcSourceAnalyzer,
            &component,
            &ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &css_source(FileId(2), css).unwrap(),
            &CssSourceName::from("stillwind.css"),
            &CompilerOptions::default(),
        )
    };

    let individual_without_package_definitions = compiler(
        "@import \"stillwind/theme.css\" layer(theme);\n@import \"stillwind/utilities.css\" layer(utilities);",
    );
    assert!(
        individual_without_package_definitions
            .lint_findings
            .iter()
            .any(|finding| matches!(finding.diagnostic, LintDiagnostic::UnknownUtility { .. }))
    );

    let utilities_only = compiler(
        "@import \"stillwind/theme.css\" layer(theme);\n@import \"stillwind/utilities.css\" layer(utilities);\n@import \"stillwind/builtins.css\";",
    );
    assert_eq!(utilities_only.diagnostics, [] as [CompileError; 0]);
    assert!(!utilities_only.css.contains("box-sizing:border-box"));
    assert!(!utilities_only.css.contains("@property --tw-"));
    let theme_start = utilities_only.css.find("@layer theme{").unwrap() + "@layer theme{".len();
    let selector_start = skip_css_trivia(&utilities_only.css, theme_start);
    assert!(normalize_css(&utilities_only.css[selector_start..]).starts_with(":root,:host{"));
    assert!(utilities_only.css.contains("@layer utilities{"));

    let full = compiler("@import \"stillwind\";");
    assert_eq!(full.diagnostics, [] as [CompileError; 0]);
    assert!(full.css.contains("box-sizing: border-box"));
    assert!(!full.css.contains("@property --tw-"));

    let tailwind_individual = compiler(
        "@import \"stillwind/theme.css\" layer(theme);\n@import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
    );
    assert_eq!(tailwind_individual.diagnostics, [] as [CompileError; 0]);
    assert!(tailwind_individual.css.contains("@layer utilities{"));

    let content = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &SourceFile::new(
            FileId(3),
            None,
            None,
            "import { style } from \"stillwind/react\"; const props = style(\"bg-linear-to-r\");",
        ).unwrap(),
    &ModuleId::from("test.tsx"),
    ScriptLanguage::Tsx,
    ScriptModuleKind::Unambiguous,
    &css_source(
            FileId(4),
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ).unwrap(),
        &CssSourceName::from("stillwind.css")
    , &CompilerOptions::default());
    assert_eq!(content.diagnostics, [] as [CompileError; 0]);
    assert!(!content.css.contains("@property --tw-gradient-position"));
}
