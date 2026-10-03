//! Integration of import binding, declarations, reference resolution and module summaries.
//!
//! Every test here goes through the public [`parse`] entry point with real source
//! text, because most of the contracts are about what a module looks like from
//! the outside: which framework was selected, which spans are recorded, which
//! exports denote assets, and which shapes are rejected.

use std::collections::BTreeMap;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintFinding};
use stillwind_ir::{
    AdapterKind, ClassExpression, FRAMEWORKS, GroupEntry, RuleExpression, ScriptLanguage,
    ScriptModuleKind, StyleCall, StyleInput, StyleVariant,
};
use stillwind_js_oxc::parse;
use stillwind_semantic::{AssetKind, ExportedValue, ImportBinding};
use stillwind_source::{FileId, ModuleId, SourceFile, Span};

fn rule(expression: &ClassExpression) -> &RuleExpression {
    let ClassExpression::Rule(rule) = expression else {
        panic!("rule")
    };
    rule.as_ref()
}

#[test]
fn parser_diagnostics_keep_external_detail_as_notes() {
    let source = SourceFile::new(FileId(1), None, None, "const = ;").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let error = parsed
        .diagnostics
        .iter()
        .find(|error| error.diagnostic == CompileDiagnostic::JavaScriptParse)
        .unwrap();
    assert_eq!(error.diagnostic, CompileDiagnostic::JavaScriptParse);
    assert_eq!(error.to_string(), "JavaScript parse error");
    assert_eq!(error.labels[0].span, source.full_span());
    assert_ne!(error.notes[0], "");
    assert_eq!(parsed.lint_findings, [] as [LintFinding; 0]);
}

/// IR span mapping visits every span and fails atomically.
///
/// A projection that fails partway must leave the original IR untouched, so callers can
/// retry with another map without repairing state.
#[test]
fn ir_span_mapping_relocates_nested_expressions_transactionally() {
    let source = SourceFile::new(FileId(1), None, None, "import {style} from 'stillwind/react'; const ref = style.ref('target'); const props = style(ref, active ? 'flex' : 'grid', {class: external, 'aria-label': label});").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    let before = parsed.module.clone();
    let mut visited = 0;
    let relocated = before
        .try_map_spans(|span| {
            visited += 1;
            source.slice(span).unwrap();
            Ok::<_, ()>(Span::new(FileId(2), span.start.0 + 17, span.end.0 + 17))
        })
        .unwrap();
    assert!(visited > 10);
    let restored = relocated
        .try_map_spans(|span| Ok::<_, ()>(Span::new(FileId(1), span.start.0 - 17, span.end.0 - 17)))
        .unwrap();
    assert_eq!(restored, before);
    let mut visited = 0;
    assert!(
        before
            .try_map_spans(|span| {
                visited += 1;
                if visited == 3 {
                    Err("ambiguous projection")
                } else {
                    Ok(span)
                }
            })
            .is_err()
    );
    assert_eq!(parsed.module, before);
}

/// Aliased macro imports are recognized, and a shadowing local is not.
#[test]
fn framework_registry_preserves_import_aliases_and_shadowing() {
    for spec in FRAMEWORKS {
        let source = SourceFile::new(
            FileId(1), None, None,
            format!(
                "import {{style as s}} from '{}'; const a = s('flex'); function f(s: Function) {{ return s('grid'); }}",
                spec.macro_import,
            ),
        ).unwrap();
        let parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.module.adapter, spec.id);
        assert_eq!(parsed.module.style_calls.len(), 1);
    }
}

/// A type-only import never selects an adapter or expands calls.
#[test]
fn type_only_macro_imports_do_not_select_a_framework_or_expand_calls() {
    for declaration in [
        "import type {style as s} from 'stillwind/react';",
        "import {type style as s} from 'stillwind/react';",
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("{declaration} const a = s('grid');"),
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
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        assert_eq!(parsed.framework, None);
        assert_eq!(parsed.module.style_calls, [] as [StyleCall; 0]);

        let mixed = SourceFile::new(
            FileId(2),
            None,
            None,
            format!(
                "{declaration} import {{style}} from 'stillwind/solid'; const b = style('flex');"
            ),
        )
        .unwrap();
        let parsed = parse(
            &mixed,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        assert_eq!(parsed.framework, Some(AdapterKind::Solid));
        assert_eq!(parsed.module.style_calls.len(), 1);
    }
}

/// Macro specifier spans, references and reserved names stay usable for edits.
///
/// The reference list must contain only real uses of the macro binding, so a host edit
/// can remove the import without touching unrelated code.
#[test]
fn import_edit_metadata_tracks_macro_references_and_preserves_mixed_bindings() {
    let source = SourceFile::new(FileId(1), None, None, "import fallback, {style as s, type StyleArgument, other} from 'stillwind/react'; const a = s('flex'); const retained = s; function f(s: Function, stillwindRef: string) { return s('grid') + stillwindClsx; }").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    let imports = &parsed.module.imports;
    assert_eq!(imports.macros.len(), 1);
    let declaration = &imports.macros[0];
    assert!(
        source
            .slice(declaration.span)
            .unwrap()
            .starts_with("import fallback")
    );
    assert_eq!(declaration.specifiers.len(), 4);
    let macros: Vec<_> = declaration
        .specifiers
        .iter()
        .filter(|specifier| specifier.is_macro)
        .collect();
    assert_eq!(macros.len(), 1);
    assert_eq!(macros[0].local, "s");
    assert_eq!(source.slice(macros[0].span).unwrap(), "style as s");
    assert_eq!(macros[0].references.len(), 2);
    for reference in &macros[0].references {
        assert_eq!(source.slice(*reference).unwrap(), "s");
    }
    for reserved in ["fallback", "StyleArgument", "stillwindRef", "stillwindClsx"] {
        assert!(imports.reserved_names.iter().any(|name| name == reserved));
    }
}

/// Specifier spans cover the literal including its quotes, even after non-ASCII text.
#[test]
fn module_specifier_spans_preserve_quotes_and_utf8_offsets() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "// 日本語 🧭\nimport {x} from '@a'; export {x as y} from \"@b\"; export * from '@c';",
    )
    .unwrap();
    let summary = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    )
    .summary;
    assert_eq!(summary.module_specifiers.len(), 3);
    for (specifier, literal) in summary
        .module_specifiers
        .iter()
        .zip(["'@a'", "\"@b\"", "'@c'"])
    {
        assert_eq!(
            &source.text()[specifier.start as usize..specifier.end as usize],
            literal
        );
    }
}

/// Only the imported macro selects the framework; two frontends are an error.
#[test]
fn selects_solid_from_the_macro_binding_and_rejects_mixed_frontends() {
    let source = SourceFile::new(FileId(1), None, None, "import type { StyleArgument } from 'stillwind/react'; import { style } from 'stillwind/solid'; const props = style('flex');").unwrap();
    let parsed = parse(
        &source,
        &ModuleId::from("test.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert_eq!(parsed.module.adapter, AdapterKind::Solid);
    assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
    let mixed = SourceFile::new(FileId(2), None, None, "import {style} from 'stillwind/solid'; import {style as reactStyle} from 'stillwind/react'; const props = style('flex');").unwrap();
    assert!(
        parse(
            &mixed,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        )
        .diagnostics
        .iter()
        .any(|error| error.diagnostic == CompileDiagnostic::MixedFrameworks)
    );
}

/// Import slices follow local aliases and drop unreachable imports.
///
/// Both `probe_imports` and per-export slices must reach through a reassigned local,
/// while an import nothing reads stays out of the result.
#[test]
fn semantic_reads_follow_local_aliases_without_unrelated_imports() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"
import { style } from 'stillwind/react';
import { first } from './first';
import { second } from './second';
import { unrelated } from './unrelated';
let selected = first;
selected = second;
const alias = selected;
const unused = unrelated;
export const attached = style(alias);
",
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
    assert_eq!(
        parsed
            .summary
            .probe_imports
            .iter()
            .map(|import| import.source.as_str())
            .collect::<Vec<_>>(),
        vec!["./first", "./second"]
    );
    assert_eq!(parsed.summary.export_imports["attached"].len(), 2);
}

/// A parameter that shadows an import contributes no probe import.
#[test]
fn semantic_reads_preserve_binding_shadowing() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r"
import { style } from 'stillwind/react';
import { handle } from './unrelated';
export function attach(handle: unknown) { return style(handle); }
",
    )
    .unwrap();
    assert_eq!(
        parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        )
        .summary
        .probe_imports,
        [] as [ImportBinding; 0]
    );
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

#[test]
fn parses_anonymous_and_labeled_ref_declarations() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const anonymous = style.ref();
const named = style.ref("trigger");
const punctuation = style.ref("項目 🧭 /:");
const empty = style.ref("");"#,
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
    assert_eq!(parsed.module.ref_definitions.len(), 4);
    for (definition, (binding, label)) in parsed.module.ref_definitions.iter().zip([
        ("anonymous", None),
        ("named", Some("trigger")),
        ("punctuation", Some("項目 🧭 /:")),
        ("empty", Some("")),
    ]) {
        assert_eq!(definition.binding.value, binding);
        assert_eq!(
            definition.label.as_ref().map(|label| label.value.as_str()),
            label
        );
        assert_eq!(source.slice(definition.binding.span).unwrap(), binding);
        assert!(
            source
                .slice(definition.call_span)
                .unwrap()
                .starts_with("style.ref(")
        );
    }
}

/// Exports and re-exports become summary entries with normalized paths.
#[test]
fn summarizes_exported_assets_and_module_edges() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
import { external as importedExternal } from "./external";
export const menu = style.ref("menu");
const trigger = style.ref("trigger");
export { trigger as menuTrigger };
export { sharedTransition as transition } from "./shared";
export const Parent = () => <div {...style(menu)}><button {...style(trigger)} /></div>;"#,
    )
    .unwrap();

    let parsed = parse(
        &source,
        &ModuleId::from("src/Parent.tsx"),
        &[],
        &BTreeMap::default(),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.summary.module_id, "src/Parent.tsx".into());
    assert_eq!(parsed.summary.imports.len(), 1);
    assert_eq!(parsed.summary.imports[0].local, "importedExternal");
    for (exported, binding) in [("menu", "menu"), ("menuTrigger", "trigger")] {
        let Some(ExportedValue::Refs(value)) = parsed.summary.exports.get(exported) else {
            panic!("{exported} should be an exported ref");
        };
        assert_eq!(value.kind, AssetKind::Ref);
        let candidates = value.finite_candidates().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates.first().unwrap().binding_name, binding);
    }
    assert_eq!(parsed.summary.re_exports[0].exported, "transition");
    assert_eq!(parsed.summary.re_exports[0].imported, "sharedTransition");
}

/// A conditional ref choice is left for the checker instead of being widened.
#[test]
fn leaves_conditional_handle_propagation_to_typescript() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const alias = first;
export const selected = enabled ? alias : second;"#,
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
    assert!(!parsed.summary.exports.contains_key("selected"));
}

/// Spread arguments in every asset macro are reported at the spread itself.
#[test]
fn spread_arguments_in_asset_macros_report_diagnostics() {
    for (body, kind) in [
        (
            "const x=style.ref(...args);",
            CompileDiagnostic::InvalidRefArgument,
        ),
        (
            "const x=style.keyframes(...args);",
            CompileDiagnostic::InvalidKeyframesDefinition,
        ),
        (
            "const a=style.keyframes({from:'flex'}); const x=style(a(...args));",
            CompileDiagnostic::InvalidKeyframesOptions,
        ),
        (
            "const x=style.viewTransition(...args);",
            CompileDiagnostic::InvalidViewTransitionDefinition,
        ),
        (
            "const r=style.ref(); const x=style(r.asContainer(...args));",
            CompileDiagnostic::InvalidContainerType,
        ),
        (
            "const r=style.ref(); const x=style({[r.at(...args)]: 'flex'});",
            CompileDiagnostic::NonStaticRefVariant,
        ),
        (
            "const r=style.ref(); const x=style({[r.at('width > 1px',...args)]: 'flex'});",
            CompileDiagnostic::NonStaticRefVariant,
        ),
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("import {{ style }} from 'stillwind/react'; {body}"),
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
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.diagnostic == kind)
            .unwrap_or_else(|| panic!("missing {kind:?} for {body}: {:?}", parsed.diagnostics));
        assert!(
            source
                .slice(diagnostic.labels[0].span)
                .unwrap()
                .contains("...args"),
            "{body}"
        );
    }
}

#[test]
fn ref_declarations_require_literal_labels_and_supported_arity() {
    for (arguments, expected) in [
        ("label", CompileDiagnostic::InvalidRefArgument),
        ("42", CompileDiagnostic::InvalidRefArgument),
        ("`item`", CompileDiagnostic::InvalidRefArgument),
        ("'one', 'two'", CompileDiagnostic::InvalidRefArguments),
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("import {{style}} from 'stillwind/react'; const item=style.ref({arguments});"),
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
            parsed
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == expected),
            "{arguments}: {:?}",
            parsed.diagnostics
        );
        assert!(parsed.module.ref_definitions.is_empty(), "{arguments}");
    }
}

/// Attribute entries keep the span of the value expression.
#[test]
fn extracts_aria_and_data_attributes_with_value_expression_spans() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style({ "aria-busy": loading, "data-state": state });"#,
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
    let Some(StyleInput::Class(ClassExpression::Group { entries, .. })) =
        parsed.module.style_calls[0].inputs.first()
    else {
        panic!("the first input should be an attribute object");
    };
    let entries = entries
        .iter()
        .map(|entry| match entry {
            GroupEntry::Attribute(attribute) => attribute,
            GroupEntry::Class(_) | GroupEntry::CustomProperty(_) => panic!("expected attributes"),
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name.value, "aria-busy");
    assert_eq!(source.slice(entries[0].value).unwrap(), "loading");
    assert_eq!(entries[1].name.value, "data-state");
    assert_eq!(source.slice(entries[1].value).unwrap(), "state");
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

/// Raw selector keys and non-scopable wrapper values are rejected.
#[test]
fn rejects_raw_selector_keys_and_non_variant_wrapper_contents() {
    for argument in [
        r#"{ "&:hover": "flex" }"#,
        r#"{ ":": "flex" }"#,
        r#"{ "hover:": { "aria-busy": true } }"#,
        r#"{ "hover:": { class: "external" } }"#,
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
        assert!(!parsed.diagnostics.is_empty(), "{argument}");
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
fn unclaimed_keyframes_uses_report_diagnostics() {
    for body in [
        "const a=style.keyframes({from:'x'}); const use=a('');",
        "const a=style.keyframes({from:'x'}); export const use=a('');",
        "const a=style.keyframes({from:'x'}); const use=style({class:a('')});",
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("import {{style}} from 'stillwind/react'; {body}"),
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
            parsed
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == CompileDiagnostic::KeyframesUseNotAClassInput),
            "{body}: {:?}",
            parsed.diagnostics
        );
    }
}

/// A keyframes use the class lowering claims stays clean, wherever the class input sits.
#[test]
fn claimed_keyframes_uses_stay_diagnostic_free() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const a = style.keyframes({ from: "opacity-0" });
const b = style.keyframes({ to: "opacity-0" });
export const props = style("flex", enabled ? a("") : b(""), ["p-4", a("duration-180")], {
  "hover:": a(""),
});"#,
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
