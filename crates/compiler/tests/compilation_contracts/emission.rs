//! Utility emission, precedence, registration dependencies, and diagnostics.

use stillwind_css::MAX_VALUE_NESTING;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, UtilityFailure};

use super::compile;

#[test]
fn deeply_nested_utility_values_fail_with_a_nesting_diagnostic() {
    let levels = MAX_VALUE_NESTING + 1;
    let value = format!("{}1{}", "(".repeat(levels), ")".repeat(levels));
    let utility = format!("w-[calc({value})]");
    let (source, result) = compile(
        &format!("export const x = style(\"{utility}\");"),
        "@theme { --spacing: 1px; }",
    );
    let diagnostic = result.diagnostics.first().expect("a nesting diagnostic");
    assert!(
        matches!(
            diagnostic.diagnostic,
            CompileDiagnostic::CssNestingTooDeep { limit, .. }
                if limit == MAX_VALUE_NESTING
        ),
        "{:#?}",
        result.diagnostics
    );
    assert_eq!(source.slice(diagnostic.labels[0].span).unwrap(), utility);
    assert!(result.css.is_empty(), "{}", result.css);
}

#[test]
fn utility_precedence_is_independent_of_literal_and_array_order() {
    let css = "@utility a { color:red; } @utility b { color:blue; } \
               @stillwind-utility builtin { color:green; }";
    for input in [
        "'a b builtin'",
        "'builtin b a'",
        "['b', 'builtin', 'a']",
        "'b', 'a', 'builtin'",
    ] {
        let (_, result) = compile(&format!("export const x = style({input});"), css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.is_empty(), "{result:#?}");
        assert!(
            result.css.ends_with("{color:red;color:blue;color:green;}"),
            "{input}: {}",
            result.css
        );
    }
    for input in ["'hover:b hover:a'", "['hover:a', 'hover:b']"] {
        let (_, result) = compile(
            &format!("export const x = style({input});"),
            &format!("{css} @variant hover {{ &:hover {{ @slot; }} }}"),
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        let red = result.css.find("color:red;").unwrap();
        let blue = result.css.find("color:blue;").unwrap();
        assert!(red < blue, "{input}: {}", result.css);
    }
}

#[test]
fn recursive_theme_macros_reject_the_candidate_without_partial_output() {
    for value in ["--value(--color-*)", "calc(--value(--color-*) + 1)"] {
        let (_, result) = compile(
            "const x = style('x-loop');",
            &format!(
                "@utility x-* {{ color: --value(--color-*); }} @theme inline {{ --color-loop: {value}; }}"
            ),
        );
        assert!(
            result.lint_findings.iter().any(|finding| matches!(
                finding.diagnostic,
                LintDiagnostic::UnresolvedUtility {
                    classification: UtilityFailure::Unknown,
                    ..
                }
            )),
            "{result:#?}"
        );
        assert_eq!(result.css, "");
    }
}

#[test]
fn css_values_survive_emission_and_importance() {
    let (_, result) = compile(
        "const x = style('x!');",
        r#"@utility x { content: "calc(1 + 1);a"; width:calc(1px + 2px); background:url('data:text/plain;calc(1 + 1)'); }"#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(r#"content: "calc(1 + 1);a"!important;"#),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("width:calc(1px + 2px)!important"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("data:text/plain;calc(1 + 1)"),
        "{}",
        result.css
    );
}

#[test]
fn mixed_variant_forms_override_in_source_order_even_on_one_line() {
    for (definitions, expected) in [
        (
            "@variant state { &:hover { @slot; } } @custom-variant state (&:focus);",
            ":focus",
        ),
        (
            "@custom-variant state (&:focus); @variant state { &:hover { @slot; } }",
            ":hover",
        ),
        (
            "@variant state { &:hover { @slot; } } @custom-variant state { &:active { @slot; } }",
            ":active",
        ),
    ] {
        let (_, result) = compile(
            "const x = style('state:x');",
            &format!("@utility x {{ color:red; }} {definitions}"),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.css.contains(expected), "{}", result.css);
    }
}

#[test]
fn literal_values_and_explicit_fallbacks_survive_utility_emission() {
    let (_, result) = compile(
        "const x = style('literal');",
        r#"
        @stillwind-property --registered { syntax: "<length>"; inherits: false; initial-value: 0px; }
        @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @utility literal {
            content: ",var(--x) oklab(0% none none/0) deg*-1";
            transform: var(--component);
            width: var(--registered, 0px);
        }
    "#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(r#"",var(--x) oklab(0% none none/0) deg*-1""#),
        "{}",
        result.css
    );
    assert!(result.css.contains("var(--component)"), "{}", result.css);
    assert!(!result.css.contains("var(--component,)"));
    assert!(
        result.css.contains("var(--registered, 0px)"),
        "{}",
        result.css
    );
}

#[test]
fn explicit_registration_dependencies_survive_apply_and_do_not_leak_into_css() {
    let (_, result) = compile(
        "const x = style('consumer');",
        r#"
        @stillwind-property --dependency { activation: dependency; syntax: "*"; inherits: false; }
        @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @utility consumer { @stillwind-require-properties --dependency; transform:var(--dependency,); }
        .authored { @apply consumer; }
    "#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("@property --dependency").count(), 1);
    assert!(!result.css.contains("@stillwind-require-properties"));
    let (_, missing) = compile(
        "const x = style('consumer');",
        "@utility consumer { @stillwind-require-properties --missing; color:red; }",
    );
    assert_ne!(missing.diagnostics, [] as [CompileError; 0]);
}
