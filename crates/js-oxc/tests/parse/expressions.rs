//! Checks class, object, array, and variant expression lowering.

use stillwind_diagnostics::{LintDiagnostic, RuleGroupInputErrorKind};

use super::*;

fn rule(expression: &ClassExpression) -> &RuleExpression {
    let ClassExpression::Rule(rule) = expression else {
        panic!("rule")
    };
    rule.as_ref()
}

/// Only the macro-bound callee is a style call, and literal utilities cost no probe.
#[test]
fn extracts_only_an_import_bound_style_call_with_literal_spans() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style as s } from "stillwind/react";
const props = s("px-4 text-center", "hover:bg-accent");
function local(s: (value: string) => unknown) { s("not-a-style-call"); }"#,
    )
    .unwrap();

    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(
        parsed.type_probes.is_empty(),
        "literal utilities need no checker queries"
    );

    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    assert_eq!(parsed.module.style_calls.len(), 1);
    let Some(StyleInput::Class(expression)) = parsed.module.style_calls[0].inputs.first() else {
        panic!("the first input should be a static utility string");
    };
    let RuleExpression::Utilities(first) = rule(expression) else {
        panic!("utility")
    };
    assert_eq!(first.value, "px-4 text-center");
    assert_eq!(source.slice(first.span).unwrap(), "px-4 text-center");
}

/// Arrays stay arrays and variant wrappers keep their prefix text.
#[test]
fn flattens_arrays_and_retains_variant_prefixes() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style(["px-4", loading && "opacity-60"], { "hover:": "bg-accent" });"#,
    )
    .unwrap();

    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let StyleInput::Class(ClassExpression::Array(elements)) =
        &parsed.module.style_calls[0].inputs[0]
    else {
        panic!("the array should stay an array");
    };
    assert!(matches!(rule(&elements[0]), RuleExpression::Utilities(_)));
    assert!(matches!(elements[1], ClassExpression::Conditional { .. }));
    let Some(StyleInput::Class(ClassExpression::Group { entries, .. })) =
        parsed.module.style_calls[0].inputs.get(1)
    else {
        panic!("the wrapper object should be preserved in the style IR");
    };
    let GroupEntry::Class(inner) = &entries[0] else {
        panic!("expected a variant wrapper");
    };
    let RuleExpression::Variant {
        variant: StyleVariant::Prefix(prefix),
        inputs,
    } = rule(inner)
    else {
        panic!("expected a variant wrapper");
    };
    assert_eq!(prefix.value, "hover:");
    let RuleExpression::Utilities(value) = &inputs[0] else {
        panic!("expected utility")
    };
    assert_eq!(value.value, "bg-accent");
}

#[test]
fn invalid_object_entries_report_errors_or_omit_variant_values_with_lints() {
    for (argument, reason) in [
        (r#"{ "&:hover": "flex" }"#, None),
        (
            r#"{ "hover:": { class: "external" } }"#,
            Some(RuleGroupInputErrorKind::ExternalClass),
        ),
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style({argument});"
            ),
        )
        .unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        if let Some(reason) = reason {
            assert!(parsed.diagnostics.is_empty(), "{argument}");
            assert_eq!(parsed.lint_findings.len(), 1);
            assert_eq!(
                parsed.lint_findings[0].diagnostic,
                LintDiagnostic::InvalidRuleGroupInput { reason }
            );
            assert_eq!(parsed.module.style_calls[0].inputs, []);
        } else {
            assert_eq!(parsed.lint_findings.len(), 1);
            assert_eq!(
                parsed.lint_findings[0].diagnostic,
                LintDiagnostic::InvalidStyleKey
            );
        }
    }
}

/// Nested wrappers keep each utility's own source span.
#[test]
fn nested_variant_wrappers_preserve_utility_source_spans() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style({ "hover:": ["text-red", { "[&:where(.mod)]:": "text-blue" }] });"#,
    )
    .unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let StyleInput::Class(ClassExpression::Group { entries, .. }) =
        &parsed.module.style_calls[0].inputs[0]
    else {
        panic!("wrapper object")
    };
    let GroupEntry::Class(inner) = &entries[0] else {
        panic!("outer wrapper");
    };
    let RuleExpression::Variant {
        variant: StyleVariant::Prefix(outer),
        inputs: wrapped,
    } = rule(inner)
    else {
        panic!("outer wrapper")
    };
    let [wrapped] = wrapped.as_slice() else {
        panic!("one wrapped array")
    };
    let RuleExpression::Array(elements) = wrapped else {
        panic!("wrapper array")
    };
    let RuleExpression::Group {
        inputs: inner_entries,
        ..
    } = &elements[1]
    else {
        panic!("inner wrapper object")
    };
    let RuleExpression::Variant {
        variant: StyleVariant::Prefix(inner),
        inputs,
    } = &inner_entries[0]
    else {
        panic!("inner wrapper")
    };
    let RuleExpression::Utilities(value) = &inputs[0] else {
        panic!("utility")
    };
    assert_eq!(outer.value, "hover:");
    assert_eq!(inner.value, "[&:where(.mod)]:");
    assert_eq!(value.value, "text-blue");
    assert_eq!(source.slice(value.span).unwrap(), "text-blue");
}

#[test]
fn external_class_arrays_keep_leaf_occurrences_and_spread_boundaries() {
    let source = SourceFile::new(FileId(1), None, None, "import {style} from 'stillwind/react'; const props = style({class: [first, [,, second, []], flag ? [third] : [], [...values], (fourth, fifth)]});").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let StyleInput::Class(ClassExpression::Group { entries, .. }) =
        &parsed.module.style_calls[0].inputs[0]
    else {
        panic!("class group")
    };
    let GroupEntry::Class(value) = &entries[0] else {
        panic!("class entry")
    };
    let ClassExpression::ClassValue { span, arguments } = value.as_ref() else {
        panic!("external class")
    };
    assert!(source.slice(*span).unwrap().starts_with("[first,"));
    assert_eq!(
        arguments
            .iter()
            .map(|span| source.slice(*span).unwrap())
            .collect::<Vec<_>>(),
        [
            "first",
            "second",
            "flag ? [third] : []",
            "[...values]",
            "(fourth, fifth)"
        ]
    );
    let relocated = parsed
        .module
        .try_map_spans(|span| Ok::<_, ()>(Span::new(FileId(2), span.start.0 + 10, span.end.0 + 10)))
        .unwrap();
    let restored = relocated
        .try_map_spans(|span| Ok::<_, ()>(Span::new(FileId(1), span.start.0 - 10, span.end.0 - 10)))
        .unwrap();
    assert_eq!(restored, parsed.module);
}
