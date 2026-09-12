//! Structure-preserving lowering of conditional class expressions.
//!
//! Checks emitted code structure and occurrences of test expressions (without executing
//! JS), that `&&` and `?:` keep their short-circuiting and source evaluation order
//! (attribute values included), that base rules precede conditional rules in the
//! stylesheet, and that unsupported forms keep reporting their existing diagnostics.

use stillwind_compiler::CompileResult;
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

fn compile(body: &str) -> CompileResult {
    let component = SourceFile::new(
        FileId(1),
        "src/Flow.tsx",
        format!(
            "import {{ style }} from \"stillwind/react\";\n\
             declare const a: boolean;\n\
             declare const b: boolean;\n\
             declare function f(): boolean;\n\
             {body}"
        ),
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --color-muted: gray; } @utility p-4 { padding: 1rem; } \
         @utility p-2 { padding: 0.5rem; } @utility m-2 { margin: 0.5rem; } \
         @utility m-4 { margin: 1rem; }",
    )
    .unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);
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
fn attribute_values_keep_their_source_evaluation_order() {
    let result = compile("const props = style({ \"aria-busy\": f() }, b ? \"p-2\" : \"m-2\");");
    let replacement = emitted(&result);
    let attribute = replacement.find("\"aria-busy\":f()").expect("attribute");
    let classes = replacement.find("className:").expect("class property");
    assert!(attribute < classes, "{replacement}");
}

#[test]
fn arrays_and_conditional_objects_keep_their_structure() {
    let result = compile("const props = style([a ? [\"p-2\", b && \"m-2\"] : \"p-4\"]);");
    let replacement = emitted(&result);
    assert!(replacement.contains("[(a) ? (["), "{replacement}");
    let attributes =
        compile("const props = style(a ? { \"aria-busy\": f() } : { \"data-x\": 1 });");
    let replacement = emitted(&attributes);
    assert!(replacement.contains("stillwindMerge"), "{replacement}");
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
fn await_and_this_tests_keep_their_positions() {
    for (body, expected) in [
        (
            "declare function ready(): Promise<boolean>;\n\
             export async function make() { return style(await ready() ? 'p-2' : 'm-2'); }",
            "(await ready()) ? (",
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
fn unsupported_forms_still_report_their_existing_diagnostics() {
    for (body, code) in [
        ("const props = style(a || \"p-2\");", "SHJS002"),
        ("const props = style(a ?? \"p-2\");", "SHJS002"),
        (
            "const props = style(`${a ? \"hover\" : \"focus\"}:p-2`);",
            "SHJS005",
        ),
        (
            "const props = style(\"p-4\", { \"hover:\": { \"aria-busy\": true } });",
            "SHJS004",
        ),
    ] {
        let component = SourceFile::new(
            FileId(1),
            "src/Invalid.tsx",
            format!(
                "import {{ style }} from \"stillwind/react\"; declare const a: boolean; {body}"
            ),
        )
        .unwrap();
        let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();
        let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);
        assert!(
            result.diagnostics.iter().any(|error| error.code == code),
            "{body}: {:#?}",
            result.diagnostics
        );
    }
}
