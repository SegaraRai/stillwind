//! Structure-preserving lowering of conditional class expressions.
//!
//! Checks emitted code structure and occurrences of test expressions (without executing
//! JS), that `&&` and `?:` keep their short-circuiting and contribution order
//! (attribute values included), that base rules precede conditional rules in the
//! stylesheet, and that unsupported forms report their diagnostics.

use stillwind_compiler::{CompileResult, CompilerOptions, SourceTransform};
use stillwind_diagnostics::{
    CheckerFactsSubject, CompileDiagnostic, LintDiagnostic, RuleGroupInputErrorKind,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

#[test]
fn variant_omits_ref_attachment_and_composition_with_lint_findings() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const ref = style.ref();
const base = style("flex");
export const one = style({ "hover:": ref });
export const two = style({ "hover:": base });"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();
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
    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|error| matches!(
                error.diagnostic,
                LintDiagnostic::InvalidRuleGroupInput { .. }
            ))
            .count(),
        2,
        "{:#?}",
        result.diagnostics
    );
    let failures = result
        .lint_findings
        .iter()
        .filter_map(|error| match error.diagnostic {
            LintDiagnostic::InvalidRuleGroupInput { reason } => Some((
                reason,
                component.slice(error.labels[0].span).unwrap().to_owned(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        failures,
        [
            (RuleGroupInputErrorKind::RefAttachment, "ref".to_owned()),
            (RuleGroupInputErrorKind::Composition, "base".to_owned()),
        ]
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert_ne!(result.transforms, [] as [SourceTransform; 0]);
}

fn compile(body: &str) -> CompileResult {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        format!(
            "import {{ style }} from \"stillwind/react\";\n\
             declare const a: boolean;\n\
             declare const b: boolean;\n\
             declare function f(): boolean;\n\
             {body}"
        ),
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@theme { --color-accent: red; --color-muted: gray; } @utility p-4 { padding: 1rem; } \
         @utility p-2 { padding: 0.5rem; } @utility m-2 { margin: 0.5rem; } \
         @utility m-4 { margin: 1rem; }",
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
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    result
}

fn emitted(result: &CompileResult) -> &str {
    &result.transforms[0].replacement
}

#[test]
fn nested_conditionals_evaluate_each_test_once() {
    let result = compile("const props = style(\"p-4\", a && (b ? \"p-2\" : \"m-2\"));");
    let replacement = emitted(&result);
    assert_eq!(replacement.matches("(a)").count(), 1, "{replacement}");
    assert_eq!(replacement.matches("(b)").count(), 1, "{replacement}");
    assert!(replacement.contains("(a) && ((b) ? ("), "{replacement}");
    assert_eq!(result.classes.len(), 3);
}

#[test]
fn ternary_branches_keep_their_source_structure() {
    let result = compile("const props = style(a ? (b ? \"p-2\" : \"m-2\") : \"p-4\");");
    let replacement = emitted(&result);
    assert!(replacement.contains("(a) ? ((b) ? ("), "{replacement}");
    assert_eq!(replacement.matches("(a)").count(), 1, "{replacement}");
    assert_eq!(replacement.matches("(b)").count(), 1, "{replacement}");
}

#[test]
fn short_circuit_and_null_branches_lower_without_a_class() {
    let result = compile("const props = style(a ? \"p-2\" : null, b && \"m-2\");");
    let replacement = emitted(&result);
    assert!(replacement.contains(") : (null)"), "{replacement}");
    assert!(replacement.contains("(b) && ("), "{replacement}");
    assert_eq!(result.classes.len(), 2);
}

#[test]
fn clsx_flattens_outer_arrays_but_keeps_guarded_arrays_and_objects() {
    let result = compile("const props = style([a ? [\"p-2\", b && \"m-2\"] : \"p-4\"]);");
    let replacement = emitted(&result);
    assert!(
        replacement.contains("stillwindClsx((a) ? (["),
        "{replacement}"
    );
    let attributes =
        compile("const props = style(a ? { \"aria-busy\": f() } : { \"data-x\": 1 });");
    let replacement = emitted(&attributes);
    assert!(replacement.contains("\"aria-busy\":f()"), "{replacement}");
    assert!(replacement.contains("\"data-x\":1"), "{replacement}");
}

#[test]
fn base_rules_precede_conditional_rules_in_the_stylesheet() {
    let result = compile("const props = style(\"p-4\", a && \"p-2\");");
    let base = result.css.find("padding: 1rem").expect("base rule");
    let conditional = result
        .css
        .find("padding: 0.5rem")
        .expect("conditional rule");
    assert!(base < conditional, "{}", result.css);
}

#[test]
fn precomputed_async_values_and_this_keep_their_context() {
    for (body, expected) in [
        (
            "declare function ready(): Promise<boolean>;\n\
             export async function make() { const readyValue = await ready(); return style(readyValue ? 'p-2' : 'm-2'); }",
            "(readyValue) ? (",
        ),
        (
            "export class Panel { ready = true; props = style(this.ready ? 'p-2' : 'm-2'); }",
            "(this.ready) ? (",
        ),
    ] {
        let result = compile(body);
        let replacement = emitted(&result);
        assert!(replacement.contains(expected), "{replacement}");
        assert_eq!(result.classes.len(), 2);
    }
}

#[test]
fn nested_conditionals_keep_source_order_after_the_base_unit() {
    let result =
        compile("const props = style(\"p-4\", a && \"p-2\", a && (b ? \"m-2\" : \"m-4\"));");
    let order = [
        "padding: 1rem",   // unconditional base unit
        "padding: 0.5rem", // `a &&` leaf
        "margin: 0.5rem",  // nested consequent
        "margin: 1rem",    // nested alternate
    ]
    .map(|rule| {
        result
            .css
            .find(rule)
            .unwrap_or_else(|| panic!("missing {rule} in {}", result.css))
    });
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
    let guards = result
        .classes
        .iter()
        .map(|class| {
            class
                .guard_path
                .iter()
                .map(|step| if step.when_true { '+' } else { '-' })
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert_eq!(guards, ["", "+", "++", "+-"]);
}

#[test]
fn unsupported_forms_are_omitted_only_when_resolution_is_complete() {
    for (body, kind) in [
        ("const props = style(a || \"p-2\");", None),
        ("const props = style(a ?? \"p-2\");", None),
        (
            "const props = style(`${a ? \"hover\" : \"focus\"}:p-2`);",
            Some(LintDiagnostic::InvalidUtilityTemplate),
        ),
    ] {
        let component = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                "import {{ style }} from \"stillwind/react\"; declare const a: boolean; {body}"
            ),
        )
        .unwrap();
        let theme = css_source(FileId(2), "@theme {}").unwrap();
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
        if let Some(kind) = kind {
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert!(
                result
                    .lint_findings
                    .iter()
                    .any(|error| error.diagnostic == kind),
                "{body}: {:#?}",
                result.diagnostics
            );
        } else {
            assert!(
                result.diagnostics.iter().any(|error| error.diagnostic
                    == CompileDiagnostic::CheckerFactsRequired {
                        subject: CheckerFactsSubject::StyleInput
                    }),
                "{result:#?}"
            );
            assert_eq!(result.transforms, [] as [SourceTransform; 0]);
        }
    }
}

#[test]
fn custom_properties_preserve_order_and_conditional_evaluation() {
    let result = compile(
        r#"const props = style({"--x": first()}, a && {"--x": second(), "data-x": third()}, {"--y": fourth()});"#,
    );
    let code = emitted(&result);
    for value in ["first()", "second()", "third()", "fourth()"] {
        assert_eq!(code.matches(value).count(), 1, "{code}");
    }
    assert!(code.find("first()").unwrap() < code.find("second()").unwrap());
    assert!(code.find("second()").unwrap() < code.find("third()").unwrap());
    assert!(code.find("third()").unwrap() < code.find("fourth()").unwrap());
    assert!(code.contains("(a) &&"), "{code}");
    assert!(!result.css.contains("--x:"));
    assert!(!result.css.contains("--y:"));
}
