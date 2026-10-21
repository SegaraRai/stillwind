//! Checks findings about the variants and values of one candidate, their ranges, and
//! the fixes of `redundant-variant` and `prefer-typed-variables`.

use stillwind_compiler::{CompileResult, Compiler};
use stillwind_diagnostics::LintDiagnostic;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, SourceUri};

use super::*;

const THEME: &str =
    "@property --my-len { syntax: \"<length>\"; inherits: false; initial-value: 0px; }";

/// Compiles `utilities` in one `style()` call of a document with a URI, so that
/// findings can carry fixes.
fn compile_utilities(utilities: &str, options: &CompilerOptions) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(
        FileId(1),
        Some(SourceUri::parse("file:///test.tsx").unwrap()),
        None,
        format!("import {{ style }} from 'stillwind/react'; const x = style({utilities:?});"),
    )
    .unwrap();
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        format!("{} @stillwind-slot utilities;", with_defaults(THEME)),
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        options,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    (source, result)
}

/// Returns the code, covered text, and fixed utilities of each finding of `kind`.
fn findings(
    utilities: &str,
    options: &CompilerOptions,
    kind: LintKind,
) -> Vec<(String, String, Option<String>)> {
    let (source, result) = compile_utilities(utilities, options);
    result
        .lint_findings
        .iter()
        .filter(|finding| finding.kind() == kind)
        .map(|finding| {
            let fixed = finding.actions.first().map(|action| {
                let mut text = source.text().to_owned();
                for edit in action.edits.iter().rev() {
                    text.replace_range(
                        edit.range.start() as usize..edit.range.end() as usize,
                        &edit.replacement,
                    );
                }
                let start = text.find("style(\"").unwrap() + 7;
                let end = start + text[start..].find('"').unwrap();
                text[start..end].to_owned()
            });
            (
                finding.diagnostic.to_string(),
                source.slice(finding.labels[0].span).unwrap().to_owned(),
                fixed,
            )
        })
        .collect()
}

#[test]
fn redundant_variants_are_reported_at_the_variant_and_removed() {
    let options = CompilerOptions::default();
    for (utilities, covered, fixed) in [
        ("hover:hover:flex", "hover", "hover:flex"),
        ("block sm:sm:flex", "sm", "block sm:flex"),
        (
            "group-hover:group-hover:flex",
            "group-hover",
            "group-hover:flex",
        ),
        ("hover:focus:hover:flex", "hover", "hover:focus:flex"),
        ("md:sm:flex", "sm", "md:flex"),
        ("sm:md:flex", "sm", "md:flex"),
        ("@min-md:@sm:flex", "@sm", "@min-md:flex"),
    ] {
        let found = findings(utilities, &options, LintKind::RedundantVariant);
        assert_eq!(found.len(), 1, "{utilities}: {found:?}");
        assert_eq!(found[0].1, covered, "{utilities}");
        assert_eq!(found[0].2.as_deref(), Some(fixed), "{utilities}");
    }
    let (source, result) = compile_utilities("md:sm:flex", &options);
    let finding = result
        .lint_findings
        .iter()
        .find(|finding| finding.kind() == LintKind::RedundantVariant)
        .unwrap();
    assert_eq!(
        finding.labels[0].span.start.0 as usize,
        source.text().find("sm:").unwrap()
    );
    for utilities in [
        "*:*:flex",
        "hover:*:hover:flex",
        "min-[600px]:sm:flex",
        "hover:focus:flex",
    ] {
        assert_eq!(
            findings(utilities, &options, LintKind::RedundantVariant),
            [],
            "{utilities}"
        );
    }
}

#[test]
fn untyped_variables_receive_the_hint_of_the_selected_definition() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_typed_variables: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    for (utilities, covered, fixed) in [
        ("text-(--x)", "(--x)", "text-(color:--x)"),
        ("bg-(--x)", "(--x)", "bg-(color:--x)"),
        ("border-(--x)", "(--x)", "border-(color:--x)"),
        ("shadow-(--x)", "(--x)", "shadow-(shadow:--x)"),
        (
            "hover:text-[var(--x)]",
            "[var(--x)]",
            "hover:text-[color:var(--x)]",
        ),
    ] {
        let found = findings(utilities, &options, LintKind::PreferTypedVariables);
        assert_eq!(found.len(), 1, "{utilities}: {found:?}");
        assert_eq!(found[0].1, covered, "{utilities}");
        assert_eq!(found[0].2.as_deref(), Some(fixed), "{utilities}");
    }
    for utilities in ["w-(--x)", "text-(color:--x)", "text-red-500"] {
        assert_eq!(
            findings(utilities, &options, LintKind::PreferTypedVariables),
            [],
            "{utilities}"
        );
    }
    assert_eq!(
        findings(
            "text-(--x)",
            &CompilerOptions::default(),
            LintKind::PreferTypedVariables
        ),
        []
    );
}

/// A variable whose known type the property rejects reports only
/// `incompatible-property-value`, because the hint of the selected definition would
/// state the rejected type.
#[test]
fn rejected_variable_types_do_not_suggest_the_selected_hint() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_typed_variables: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    for (utilities, typed, invalid) in [
        ("bg-(--text-sm)", 0, 1),
        ("bg-(--my-len)", 0, 1),
        ("bg-(--color-red-500)", 1, 0),
    ] {
        let (_, result) = compile_utilities(utilities, &options);
        let count = |kind| {
            result
                .lint_findings
                .iter()
                .filter(|finding| finding.kind() == kind)
                .count()
        };
        assert_eq!(
            (
                count(LintKind::PreferTypedVariables),
                count(LintKind::IncompatiblePropertyValue)
            ),
            (typed, invalid),
            "{utilities}: {:?}",
            result.lint_findings
        );
    }
    assert_eq!(
        findings(
            "bg-(--color-red-500)",
            &options,
            LintKind::PreferTypedVariables
        )[0]
        .2,
        Some("bg-(color:--color-red-500)".to_owned())
    );
}

#[test]
fn contradicting_variants_and_clamped_values_cover_their_part() {
    let options = CompilerOptions::default();
    for (utilities, kind, covered) in [
        (
            "max-sm:sm:flex",
            LintKind::UnsatisfiableVariants,
            "max-sm:sm",
        ),
        (
            "hover:not-hover:flex",
            LintKind::UnsatisfiableVariants,
            "hover:not-hover",
        ),
        (
            "not-hover:hover:flex",
            LintKind::UnsatisfiableVariants,
            "not-hover:hover",
        ),
        (
            "supports-grid:focus:not-supports-grid:flex",
            LintKind::UnsatisfiableVariants,
            "supports-grid:focus:not-supports-grid",
        ),
        ("opacity-150", LintKind::ClampedValue, "150"),
        ("opacity-[1.5]", LintKind::ClampedValue, "[1.5]"),
        ("[opacity:-0.5]", LintKind::ClampedValue, "-0.5"),
        ("backdrop-grayscale-150", LintKind::ClampedValue, "150"),
        ("invert-[150%]", LintKind::ClampedValue, "[150%]"),
        (
            "grayscale-[-50%]",
            LintKind::IncompatiblePropertyValue,
            "[-50%]",
        ),
    ] {
        let found = findings(utilities, &options, kind);
        assert_eq!(found.len(), 1, "{utilities}: {found:?}");
        assert_eq!(found[0].1, covered, "{utilities}");
        assert_eq!(found[0].2, None, "{utilities}");
    }
    for utilities in [
        "sm:max-md:flex",
        "dark:portrait:flex",
        "opacity-50",
        "grayscale",
    ] {
        let (_, result) = compile_utilities(utilities, &options);
        assert!(
            !result.lint_findings.iter().any(|finding| matches!(
                finding.kind(),
                LintKind::UnsatisfiableVariants | LintKind::ClampedValue
            )),
            "{utilities}: {:?}",
            result.lint_findings
        );
    }
}

#[test]
fn variables_of_known_types_and_opacity_modifiers_report_invalid_values() {
    let options = CompilerOptions::default();
    for (utilities, covered, variable) in [
        ("bg-(--text-sm)", "(--text-sm)", true),
        ("w-(--color-red-500)", "(--color-red-500)", true),
        ("bg-(--my-len)", "(--my-len)", true),
        ("bg-red-500/150", "150", false),
        ("bg-red-500/[150%]", "[150%]", false),
    ] {
        let (source, result) = compile_utilities(utilities, &options);
        let found = result
            .lint_findings
            .iter()
            .filter(|finding| finding.kind() == LintKind::IncompatiblePropertyValue)
            .collect::<Vec<_>>();
        assert_eq!(found.len(), 1, "{utilities}: {:?}", result.lint_findings);
        assert_eq!(source.slice(found[0].labels[0].span).unwrap(), covered);
        assert_eq!(
            matches!(
                found[0].diagnostic,
                LintDiagnostic::VariableTypeMismatch { .. }
            ),
            variable,
            "{utilities}"
        );
    }
    let (_, result) = compile_utilities("bg-red-500/[150%]", &options);
    assert!(
        result
            .css
            .contains("var(--color-red-500) 150%, transparent)"),
        "{}",
        result.css
    );
}

/// Lint settings select reports only; every rule leaves the CSS unchanged.
#[test]
fn candidate_lints_do_not_change_output() {
    let utilities = "hover:hover:flex md:sm:block max-sm:sm:grid opacity-150 bg-(--text-sm) bg-red-500/[150%] text-(--x)";
    let all = CompilerOptions {
        lint: LintOverrides {
            prefer_typed_variables: Some(LintSeverity::Deny),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    let none = CompilerOptions {
        lint: LintOverrides {
            redundant_variant: Some(LintSeverity::Off),
            unsatisfiable_variants: Some(LintSeverity::Off),
            clamped_value: Some(LintSeverity::Off),
            incompatible_property_value: Some(LintSeverity::Off),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    let (_, reported) = compile_utilities(utilities, &all);
    let (_, silent) = compile_utilities(utilities, &none);
    assert_eq!(reported.css, silent.css);
    assert!(
        reported.lint_findings.len() >= 6,
        "{:?}",
        reported.lint_findings
    );
}
