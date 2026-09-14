//! Public compiler contracts for fragment layouts, effect proofs, and source attribution.
//!
//! Cases compile authored calls through the frontend, then inspect output, helper
//! bindings, typed findings, and independently converted source coordinates.

use stillwind_compiler::{CompileResult, render_transforms};
use stillwind_diagnostics::LintKind;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile};

use super::Compiler;

/// Compiles a call body under one framework import and an empty theme.
///
/// # Panics
///
/// Panics when a source/fixture cannot be constructed or compilation reports errors.
fn compile(framework: &str, body: &str) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(
        FileId(1),
        "src/Folding.tsx",
        format!("import {{ style }} from 'stillwind/{framework}';\n{body}"),
    )
    .unwrap();
    let css = crate::css_fixture::source(FileId(2), "style.css", "@theme {}").unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &css);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    (source, result)
}

/// Checks the same custom-property call against object-style and string-style framework outputs.
#[test]
fn custom_properties_materialize_directly_for_every_framework() {
    for fw in ["react", "preact", "solid", "vue", "svelte", "astro"] {
        let (source, result) = compile(
            fw,
            r#"const props = style("left-(--progress)", { "--progress": progress });"#,
        );
        let transformed = result.apply_to_source(&source).unwrap();
        let replacement = &result.transforms[0].replacement;
        assert!(
            !transformed.contains("stillwindMerge"),
            "{fw}: {transformed}"
        );
        assert!(!transformed.contains("stillwindCustomPropertiesKey"));
        assert!(!transformed.contains("stillwindClassKey"));
        assert_eq!(
            replacement.matches("stillwindCssValue(progress)").count(),
            1
        );
        assert!(replacement.contains(if fw == "react" {
            "className:"
        } else {
            "class:"
        }));
        if matches!(fw, "svelte" | "astro") {
            assert!(
                replacement.contains("style:\"--progress:\"+"),
                "{replacement}"
            );
        } else {
            assert!(replacement.contains("style:{"), "{replacement}");
        }
        assert!(result.css.contains("left:var(--progress)"));
        assert!(
            result.lint_findings.is_empty(),
            "{:?}",
            result.lint_findings
        );
    }
}

/// Pins both the public lint code and the three source occurrences explaining a grouping obstruction.
#[test]
fn order_rejection_reports_one_typed_finding_with_source_labels() {
    let (source, result) = compile(
        "react",
        r#"const props = style({ "--x": f() }, { "data-value": g() }, { "--y": h() });"#,
    );
    let findings = result
        .lint_findings
        .iter()
        .filter(|finding| matches!(finding.kind, LintKind::StyleFoldEvaluationOrder))
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code(), "style-fold-evaluation-order");
    let spans = findings[0]
        .labels
        .iter()
        .map(|label| source.slice(label.span).unwrap())
        .collect::<Vec<_>>();
    assert!(spans[0].starts_with("style("));
    assert_eq!(&spans[1..], ["f()", "g()", "h()"]);
    let code = &result.transforms[0].replacement;
    assert!(code.contains("stillwindMerge"));
    assert!(code.find("f()").unwrap() < code.find("g()").unwrap());
    assert!(code.find("g()").unwrap() < code.find("h()").unwrap());
}

/// Applies existing lint directives to a folding finding produced after initial analysis.
#[test]
fn folding_findings_obey_existing_suppression() {
    for directive in [
        "// swlint-disable-next-line style-fold-evaluation-order",
        "/* swlint-disable style-fold-evaluation-order */",
    ] {
        let (_, result) = compile(
            "react",
            &format!(
                "{directive}\nconst props = style({{ '--x': f() }}, {{ 'data-value': g() }}, {{ '--y': h() }});"
            ),
        );
        assert!(
            result.lint_findings.is_empty(),
            "{:?}",
            result.lint_findings
        );
        assert!(result.transforms[0].replacement.contains("stillwindMerge"));
    }
}

/// Distinguishes serialization limits from effect-order obstructions without losing repeated calls.
#[test]
fn unimpeded_and_duplicate_records_do_not_report_order_findings() {
    for fw in ["react", "svelte"] {
        for body in [
            "const props = style({ '--x': f() }, { '--y': g() });",
            "const props = style({ '--x': f() }, { '--x': g() });",
        ] {
            let (_, result) = compile(fw, body);
            assert!(result.lint_findings.is_empty());
            let code = &result.transforms[0].replacement;
            assert_eq!(code.matches("f()").count(), 1);
            assert_eq!(code.matches("g()").count(), 1);
            assert!(code.find("f()").unwrap() < code.find("g()").unwrap());
        }
    }
}

/// Checks symbolic child records under a guard that must remain outside final props materialization.
#[test]
fn conditional_children_fold_without_materializing_the_parent() {
    let (_, result) = compile(
        "svelte",
        "const props = style(flag ? { '--x': f(), '--y': g() } : { '--x': h() });",
    );
    let code = &result.transforms[0].replacement;
    assert!(code.contains("stillwindMerge"));
    assert_eq!(code.matches("(flag)").count(), 1);
    assert_eq!(
        code.matches("stillwindCustomPropertiesKey").count(),
        2,
        "{code}"
    );
    assert!(!code.contains("style:"));
    assert!(result.lint_findings.is_empty());
}

/// Exercises source expressions whose meaning depends on the enclosing async or class context.
#[test]
fn direct_custom_values_keep_await_and_this_in_the_original_context() {
    for body in [
        "async function make() { return style({ '--x': await read() }); }",
        "class Component { props = style({ '--x': this.value }); }",
    ] {
        let (source, result) = compile("svelte", body);
        let code = result.apply_to_source(&source).unwrap();
        assert!(!code.contains("stillwindMerge"));
        assert!(code.contains(if body.starts_with("async") {
            "await read()"
        } else {
            "this.value"
        }));
    }
}

/// Compares CSS allocation with and without a runtime custom-property contribution.
#[test]
fn css_and_class_identity_do_not_depend_on_property_folding() {
    let (_, plain) = compile("react", "const props = style('flex');");
    let (_, custom) = compile("react", "const props = style('flex', { '--x': f() });");
    assert_eq!(plain.css, custom.css);
    assert_eq!(plain.classes[0].class_name, custom.classes[0].class_name);
    assert_eq!(plain.classes[0].guard_path, custom.classes[0].guard_path);
}

/// Reserves the default helper name to verify that deferred rendering respects binding collisions.
#[test]
fn deferred_helpers_remain_binding_safe() {
    let (source, result) = compile(
        "react",
        "const stillwindCssValue = 1; const props = style({ '--x': value });",
    );
    let code = result.apply_to_source(&source).unwrap();
    assert!(
        code.contains("stillwindCssValue as stillwindCssValue_1"),
        "{code}"
    );
    assert!(code.contains("stillwindCssValue_1(value)"));
    assert!(!code.contains("stillwindMerge"));
}

/// Keeps effectful tests when both arms are empty and retains values following an empty slot.
#[test]
fn empty_branches_do_not_erase_tests_or_later_contributions() {
    for expression in [
        "test() ? [] : []",
        "test() && []",
        "test() ? [[], { '--x': value() }] : []",
    ] {
        let (_, result) = compile("react", &format!("const props = style({expression});"));
        let code = &result.transforms[0].replacement;
        assert_eq!(code.matches("test()").count(), 1, "{code}");
        if expression.contains("value()") {
            assert_eq!(code.matches("value()").count(), 1, "{code}");
        }
    }
}

/// Exercises identifiers, property reads, and PURE calls across interleaved channels in every framework.
#[test]
fn pure_records_can_reorder_evaluations_but_keep_runtime_key_order() {
    for fw in ["react", "preact", "solid", "vue", "svelte", "astro"] {
        for value in ["x", "object.value", "/*#__PURE__*/ f(x)"] {
            let (_, result) = compile(
                fw,
                &format!(
                    "const props = style({{ '--x': {value} }}, {{ 'data-value': data }}, {{ '--y': y }}, 'flex');"
                ),
            );
            let code = &result.transforms[0].replacement;
            assert!(!code.contains("stillwindMerge"), "{fw}: {value}: {code}");
            assert!(
                result.lint_findings.is_empty(),
                "{value}: {:?}",
                result.lint_findings
            );
            assert!(
                code.find("class").unwrap() < code.find("data-value").unwrap(),
                "{code}"
            );
            assert!(
                code.find("data-value").unwrap() < code.find("style:").unwrap(),
                "{code}"
            );
        }
    }
}

/// Places a write or call between pure reads to check conservative order rejection.
#[test]
fn mixed_effects_keep_reads_on_their_original_side_of_writes_and_calls() {
    for value in ["f()", "f?.()", "x++", "x = 2", "/*#__PURE__*/ f(g())"] {
        let (_, result) = compile(
            "react",
            &format!(
                "const props = style({{ '--x': x }}, {{ 'data-value': {value} }}, {{ '--y': x }});"
            ),
        );
        let code = &result.transforms[0].replacement;
        assert!(code.contains("stillwindMerge"), "{value}: {code}");
        assert!(
            result
                .lint_findings
                .iter()
                .any(|finding| matches!(finding.kind, LintKind::StyleFoldEvaluationOrder)),
            "{value}: {:?}",
            result.lint_findings
        );
    }
}

/// Compares nested empty pure guards with calls, updates, and impure arguments to PURE calls.
#[test]
fn empty_pure_conditions_are_removed_recursively_without_erasing_effectful_tests() {
    for test in ["flag", "object.value", "/*#__PURE__*/ check(flag)"] {
        for expression in [
            format!("{test} ? [] : []"),
            format!("{test} && []"),
            format!("outer ? [{test} && []] : []"),
            format!("[{test} ? [] : [], 'flex']"),
        ] {
            let (_, result) = compile("react", &format!("const props = style({expression});"));
            let code = &result.transforms[0].replacement;
            assert!(
                !code.contains("flag")
                    && !code.contains("object.value")
                    && !code.contains("check("),
                "{expression}: {code}"
            );
            assert!(!code.contains("outer"), "{expression}: {code}");
        }
    }
    for test in [
        "check()",
        "check?.()",
        "flag++",
        "/*#__PURE__*/ check(effect())",
    ] {
        let (_, result) = compile("react", &format!("const props = style({test} ? [] : []);"));
        let code = &result.transforms[0].replacement;
        assert!(
            code.contains(if test.contains("effect()") {
                "effect()"
            } else {
                test
            }),
            "{test}: {code}"
        );
    }
}

/// Compares UTF-16 slices independently for reordered duplicates, guards, bases, and Unicode values.
#[test]
fn source_mappings_follow_reordered_occurrences_and_conditional_tests() {
    for fw in ["react", "preact", "solid", "vue", "svelte", "astro"] {
        for body in [
            "// 😀\r\nconst props = style({ '--x': same },\r\n { 'data-value': other },\r\n { '--y': same }, 'flex');",
            "const props = style(test() ? { '--x': f() } : { '--y': g() });",
            "const props = style('flex', flag && 'block', { class: classes });",
            "const base = style('flex'); const props = style(base, { '--x': value });",
            "const props = style({ '--x': `😀${value}\n日本語` });",
        ] {
            let (source, result) = compile(fw, body);
            let rendered = render_transforms(&source, &result.transforms).unwrap();
            let source_utf16 = source.text().encode_utf16().collect::<Vec<_>>();
            let generated_utf16 = rendered.code.encode_utf16().collect::<Vec<_>>();
            let expressions = rendered
                .mappings
                .iter()
                .flat_map(|mapping| &mapping.expressions)
                .collect::<Vec<_>>();
            assert!(!expressions.is_empty(), "{body}");
            for expression in &expressions {
                assert!(expression.exact);
                assert_eq!(
                    &generated_utf16
                        [expression.generated_start as usize..expression.generated_end as usize],
                    &source_utf16
                        [expression.original_start as usize..expression.original_end as usize],
                    "{fw}: {body}"
                );
            }
            if body.contains("other") {
                let originals = expressions
                    .iter()
                    .map(|mapping| mapping.original_start)
                    .collect::<Vec<_>>();
                assert!(
                    originals.windows(2).any(|pair| pair[0] > pair[1]),
                    "{originals:?}"
                );
                let same = expressions
                    .iter()
                    .filter(|mapping| {
                        String::from_utf16(
                            &source_utf16
                                [mapping.original_start as usize..mapping.original_end as usize],
                        )
                        .unwrap()
                            == "same"
                    })
                    .collect::<Vec<_>>();
                assert_eq!(same.len(), 2);
                assert_ne!(same[0].original_start, same[1].original_start);
            }
            if body.contains("test()") {
                assert_eq!(expressions.len(), 3);
            }
        }
    }
}

/// Checks authored grouping in a custom-property value remains part of its mapped expression.
#[test]
fn custom_values_keep_authored_parentheses_and_source_mapping() {
    for expression in [
        "(first(), second())",
        "((first(), second()))",
        "(flag ? left : right)",
        "(() => 1)",
    ] {
        let (source, result) = compile(
            "react",
            &format!("const props = style({{ '--x': {expression} }});"),
        );
        let transform = &result.transforms[0];
        assert!(
            transform
                .replacement
                .contains(&format!("stillwindCssValue({expression})")),
            "{}",
            transform.replacement
        );
        assert!(
            transform.replacement.contains("\"--x\":"),
            "{}",
            transform.replacement
        );
        assert!(!transform.replacement.contains("[\"--x\"]"));
        assert_eq!(transform.expressions.len(), 1);
        let mapping = &transform.expressions[0];
        assert_eq!(source.slice(mapping.original).unwrap(), expression);
        assert_eq!(
            &transform.replacement[mapping.start..mapping.end],
            expression
        );
    }
}
