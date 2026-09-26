//! Ref-variant tests: substitution forms, spans, and the rejected shapes.
//!
//! Ref variants are the one place a JavaScript expression may appear inside a utility
//! string, so the tests pin down which expressions count, which spans are recorded, and
//! which near-misses are errors.

use stillwind_ir::{
    ClassExpression, GroupEntry, RuleExpression, ScriptLanguage, ScriptModuleKind, StyleInput,
    StyleVariant,
};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, SourceFile, Span};

fn rule(expression: &ClassExpression) -> &RuleExpression {
    let ClassExpression::Rule(rule) = expression else {
        panic!("rule")
    };
    rule.as_ref()
}

/// `trigger()` keeps its identity in both the tag and object-key forms.
#[test]
fn argument_free_ref_variants_retain_identity_in_both_forms() {
    for expression in [
        r"`${trigger()}:text-accent`",
        r#"{ [trigger()]: "text-accent" }"#,
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const trigger = style.ref('trigger'); const props = style({expression});"
        )).unwrap();
        let parsed = parse(
            &source,
            "test.tsx",
            &[],
            &std::collections::BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let wrapper = match &parsed.module.style_calls[0].inputs[0] {
            StyleInput::Class(expression @ ClassExpression::Rule(_)) => expression,
            StyleInput::Class(ClassExpression::Group { entries, .. }) => {
                let GroupEntry::Class(inner) = &entries[0] else {
                    panic!("wrapper object")
                };
                inner.as_ref()
            }
            _ => panic!("class expression"),
        };
        let RuleExpression::Variant {
            variant: StyleVariant::Ref { refs, prefix },
            ..
        } = rule(wrapper)
        else {
            panic!("ref variant")
        };
        assert!(prefix.value.is_empty());
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].binding.value, "trigger");
        assert_eq!(source.slice(prefix.span).unwrap(), "trigger()");
    }
}

/// Interpolated templates keep ref identity and original utility spans.
///
/// Spans must be relocatable too, so the test shifts the whole IR and checks that ref
/// and prefix spans moved with it.
#[test]
fn template_interpolation_retains_ref_identity_and_original_utility_spans() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import {style} from "stillwind/react";
// 日本語 🧭
const trigger = style.ref("trigger");
const props = style(`md:${trigger`hover:`}:active:text-accent p-4`);"#,
    )
    .unwrap();
    let parsed = parse(
        &source,
        "test.tsx",
        &[],
        &std::collections::BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let inputs = &parsed.module.style_calls[0].inputs;
    let StyleInput::Class(expression) = &inputs[0] else {
        panic!("class")
    };
    let RuleExpression::Variant {
        variant: StyleVariant::Prefix(prefix),
        inputs: nested,
    } = rule(expression)
    else {
        panic!("ordinary variant")
    };
    assert_eq!(source.slice(prefix.span).unwrap(), "md:");
    let RuleExpression::Variant {
        variant: StyleVariant::Ref { refs, prefix },
        inputs: nested,
    } = &nested[0]
    else {
        panic!("ref variant")
    };
    assert_eq!(source.slice(prefix.span).unwrap(), "hover:");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].binding.value, "trigger");
    let RuleExpression::Utilities(utility) = &nested[0] else {
        panic!("utility")
    };
    assert_eq!(source.slice(utility.span).unwrap(), "active:text-accent");
    let StyleInput::Class(expression) = &inputs[1] else {
        panic!("class")
    };
    let RuleExpression::Utilities(plain) = rule(expression) else {
        panic!("plain utility")
    };
    assert_eq!(source.slice(plain.span).unwrap(), "p-4");

    let shifted = parsed
        .module
        .try_map_spans::<()>(|span| Ok(Span::new(span.file, span.start.0 + 10, span.end.0 + 10)))
        .unwrap();
    let StyleInput::Class(expression) = &shifted.style_calls[0].inputs[0] else {
        panic!("class")
    };
    let RuleExpression::Variant { inputs: nested, .. } = rule(expression) else {
        panic!("wrapper")
    };
    let RuleExpression::Variant {
        variant:
            StyleVariant::Ref {
                refs: moved,
                prefix: moved_prefix,
            },
        ..
    } = &nested[0]
    else {
        panic!("ref")
    };
    assert_eq!(moved_prefix.span.start.0, prefix.span.start.0 + 10);
    assert_eq!(
        moved[0].binding.span.start.0,
        refs[0].binding.span.start.0 + 10
    );
}

/// Near-miss substitutions are rejected rather than interpreted.
///
/// Non-ref substitutions, missing separators, partial utilities, multiple refs in one
/// segment, spread arguments and optional calls all fail.
#[test]
fn utility_templates_require_direct_ref_variant_segments() {
    for expression in [
        r#"`${"hover"}:text-accent`"#,
        r"`${trigger`hover:`}text-accent`",
        r"`text-${trigger`hover:`}:accent`",
        r"`${trigger`hover:`} ${trigger`active:`}:text-accent`",
        r"`${trigger`hover:`}`",
        r#"`${trigger`hover:${"active"}`}:text-accent`"#,
        r"`${trigger(...variants)}:text-accent`",
        r#"`${trigger("hover", "focus")}:text-accent`"#,
        r#"`${trigger?.("hover")}:text-accent`"#,
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const trigger = style.ref(); const props = style({expression});"
        )).unwrap();
        assert!(
            !parse(
                &source,
                "test.tsx",
                &[],
                &std::collections::BTreeMap::default(),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous
            )
            .diagnostics
            .is_empty(),
            "{expression}"
        );
    }
}

/// A template stored in a variable is not followed into the `style()` call.
#[test]
fn saved_interpolated_strings_are_not_followed() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import {style} from "stillwind/react";
const trigger = style.ref();
const classes = `${trigger`hover:`}:text-accent`;
const props = style(classes);"#,
    )
    .unwrap();
    let parsed = parse(
        &source,
        "test.tsx",
        &[],
        &std::collections::BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(!parsed.diagnostics.is_empty());
    assert!(parsed.module.style_calls[0].inputs.is_empty());
}
