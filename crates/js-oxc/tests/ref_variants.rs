//! Ref-variant tests: substitution forms, spans, and the rejected shapes.
//!
//! Ref variants are the one place a JavaScript expression may appear inside a utility
//! string, so the tests pin down which expressions count, which spans are recorded, and
//! which near-misses are errors.

use std::collections::BTreeMap;

use stillwind_diagnostics::CompileDiagnostic;
use stillwind_ir::{
    ClassExpression, GroupEntry, ModuleIr, RefCondition, RuleExpression, ScriptLanguage,
    ScriptModuleKind, StyleInput, StyleVariant,
};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, ModuleId, SourceFile, Span};

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
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
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
            variant:
                StyleVariant::Ref {
                    condition: RefCondition { refs, prefix },
                    scopes,
                },
            ..
        } = rule(wrapper)
        else {
            panic!("ref variant")
        };
        assert!(scopes.is_empty());
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
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
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
        variant:
            StyleVariant::Ref {
                condition: RefCondition { refs, prefix },
                scopes,
            },
        inputs: nested,
    } = &nested[0]
    else {
        panic!("ref variant")
    };
    assert_eq!(source.slice(prefix.span).unwrap(), "hover:");
    assert!(scopes.is_empty());
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
                condition:
                    RefCondition {
                        refs: moved,
                        prefix: moved_prefix,
                    },
                scopes: _,
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
                &ModuleId::from("test.tsx"),
                &[],
                &BTreeMap::default(),
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
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(!parsed.diagnostics.is_empty());
    assert!(parsed.module.style_calls[0].inputs.is_empty());
}

fn scoped_variant(input: &str) -> (SourceFile, StyleVariant, ModuleIr) {
    let source = SourceFile::new(FileId(1), None, None, format!(
        "import {{style}} from 'stillwind/react'; const trigger=style.ref('trigger'); const parent=style.ref('parent'); const outer=style.ref('outer'); const props=style({input});"
    )).unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::new(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "{input}: {:?}",
        parsed.diagnostics
    );
    let StyleInput::Class(expression) = &parsed.module.style_calls[0].inputs[0] else {
        panic!("class input: {input}");
    };
    let wrapper = match expression {
        ClassExpression::Group { entries, .. } => {
            let GroupEntry::Class(expression) = &entries[0] else {
                panic!("class entry: {input}");
            };
            expression.as_ref()
        }
        expression => expression,
    };
    let RuleExpression::Variant { variant, .. } = rule(wrapper) else {
        panic!("variant: {input}");
    };
    let variant = variant.clone();
    (source, variant, parsed.module)
}

#[test]
fn ref_scopes_accept_calls_tags_and_presence_shorthand() {
    for expression in [
        "trigger().in(parent)",
        "trigger.in(parent)",
        "trigger``.in(parent)",
    ] {
        for input in [
            format!("{{[{expression}]: 'block'}}"),
            format!("`${{{expression}}}:block`"),
        ] {
            let (_, variant, _) = scoped_variant(&input);
            let StyleVariant::Ref { condition, scopes } = variant else {
                panic!("ref: {input}");
            };
            assert_eq!(condition.refs[0].binding.value, "trigger");
            assert!(condition.prefix.value.is_empty());
            assert_eq!(scopes.len(), 1);
            assert_eq!(scopes[0].refs[0].binding.value, "parent");
            assert!(scopes[0].prefix.value.is_empty());
        }
    }
}

#[test]
fn stateful_scope_conditions_keep_prefixes_spans_and_chain_order() {
    for expression in [
        "trigger('hover').in(parent('focus')).in(outer`active`)",
        "trigger`hover`.in(parent`focus`).in(outer('active'))",
        "(trigger('hover')).in((parent('focus'))).in(outer('active'))",
    ] {
        let (source, variant, module) = scoped_variant(&format!("{{[{expression}]: 'block'}}"));
        let StyleVariant::Ref { condition, scopes } = &variant else {
            panic!("ref variant");
        };
        assert_eq!(condition.prefix.value, "hover:");
        assert_eq!(scopes.len(), 2);
        for (scope, binding, prefix) in [
            (&scopes[0], "parent", "focus:"),
            (&scopes[1], "outer", "active:"),
        ] {
            assert_eq!(scope.refs[0].binding.value, binding);
            assert_eq!(scope.prefix.value, prefix);
            assert_eq!(source.slice(scope.refs[0].binding.span).unwrap(), binding);
            assert_eq!(
                source
                    .slice(scope.prefix.span)
                    .unwrap()
                    .trim_matches(['\'', '"']),
                prefix.trim_end_matches(':')
            );
        }
        let shifted = module
            .try_map_spans::<()>(|span| {
                Ok(Span::new(span.file, span.start.0 + 10, span.end.0 + 10))
            })
            .unwrap();
        let StyleInput::Class(ClassExpression::Group { entries, .. }) =
            &shifted.style_calls[0].inputs[0]
        else {
            panic!("shifted class group");
        };
        let GroupEntry::Class(expression) = &entries[0] else {
            panic!("shifted class entry");
        };
        let RuleExpression::Variant {
            variant: StyleVariant::Ref {
                scopes: shifted, ..
            },
            ..
        } = rule(expression)
        else {
            panic!("shifted ref variant");
        };
        for (before, after) in scopes.iter().zip(shifted) {
            assert_eq!(after.prefix.span.start.0, before.prefix.span.start.0 + 10);
            assert_eq!(
                after.refs[0].binding.span.start.0,
                before.refs[0].binding.span.start.0 + 10
            );
        }
    }
}

#[test]
fn ref_scopes_reject_invalid_arguments_and_nested_scope_operands() {
    for (expression, expected) in [
        (
            "trigger().in()",
            CompileDiagnostic::InvalidRefScopeArguments,
        ),
        (
            "trigger().in(parent, outer)",
            CompileDiagnostic::InvalidRefScopeArguments,
        ),
        (
            "trigger().in(...parents)",
            CompileDiagnostic::InvalidRefScopeArguments,
        ),
        (
            "trigger().in(parent().in(outer))",
            CompileDiagnostic::NestedRefScope,
        ),
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const trigger=style.ref(); const parent=style.ref(); const outer=style.ref(); const props=style({{[{expression}]: 'block'}});"
        )).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == expected),
            "{expression}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn container_queries_and_projections_cannot_supply_ref_variant_receivers_or_scopes() {
    for expression in [
        "trigger.at().in(parent)",
        "trigger().in(parent).at('width > 1px')",
        "trigger.at().at('width > 1px')",
        "trigger().at('width > 1px')",
        "trigger().in(parent.at())",
        "trigger().in(parent.at`width > 1px`)",
        "trigger().in(parent.asAnchor())",
        "trigger().in('parent')",
        "trigger().in([parent])",
        "trigger().in(true)",
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const trigger=style.ref(); const parent=style.ref(); const props=style({{[{expression}]: 'block'}});"
        )).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(
            parsed.diagnostics.iter().any(|error| matches!(
                error.diagnostic,
                CompileDiagnostic::InvalidRefVariantReceiver
                    | CompileDiagnostic::InvalidRefScopeArguments
                    | CompileDiagnostic::UnidentifiedRefVariant
            )),
            "{expression}: {:?}",
            parsed.diagnostics
        );
    }
}
