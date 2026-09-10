//! Integration of import binding, declarations, reference resolution and module summaries.

use stillwind_ir::{AdapterKind, StyleInput};
use stillwind_js_oxc::parse;
use stillwind_semantic::ExportedValue;
use stillwind_source::{FileId, SourceFile};

#[test]
fn ir_span_mapping_relocates_nested_expressions_transactionally() {
    let source = SourceFile::new(FileId(1), "entry.tsx", "import {style} from 'stillwind/react'; const ref = style.ref('target'); const props = style(ref, active ? 'flex' : 'grid', {class: external, 'aria-label': label});").unwrap();
    let parsed = parse(&source);
    assert!(parsed.diagnostics.is_empty());
    let before = parsed.module.clone();
    let mut visited = 0;
    let relocated = before
        .try_map_spans(|span| {
            visited += 1;
            source.slice(span).unwrap();
            Ok::<_, ()>(stillwind_source::Span::new(
                FileId(2),
                span.start.0 + 17,
                span.end.0 + 17,
            ))
        })
        .unwrap();
    assert!(visited > 10);
    let restored = relocated
        .try_map_spans(|span| {
            Ok::<_, ()>(stillwind_source::Span::new(
                FileId(1),
                span.start.0 - 17,
                span.end.0 - 17,
            ))
        })
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

#[test]
fn framework_registry_preserves_import_aliases_and_shadowing() {
    for spec in stillwind_ir::framework::FRAMEWORKS {
        let source = SourceFile::new(
            FileId(1),
            "entry.tsx",
            format!(
                "import {{style as s}} from '{}'; const a = s('flex'); function f(s: Function) {{ return s('grid'); }}",
                spec.macro_import,
            ),
        ).unwrap();
        let parsed = parse(&source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.module.adapter, spec.id);
        assert_eq!(parsed.module.style_calls.len(), 1);
    }
}

#[test]
fn module_specifier_spans_preserve_quotes_and_utf8_offsets() {
    let source = SourceFile::new(
        FileId(1),
        "entry.ts",
        "// 日本語 🧭\nimport {x} from '@a'; export {x as y} from \"@b\"; export * from '@c';",
    )
    .unwrap();
    let summary = parse(&source).summary;
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

#[test]
fn selects_solid_from_the_macro_binding_and_rejects_mixed_frontends() {
    let source = SourceFile::new(FileId(1), "solid.tsx", "import type { StyleArgument } from 'stillwind/react'; import { style } from 'stillwind/solid'; const props = style('flex');").unwrap();
    let parsed = parse(&source);
    assert_eq!(parsed.module.adapter, AdapterKind::Solid);
    assert!(parsed.diagnostics.is_empty());
    let mixed = SourceFile::new(FileId(2), "mixed.tsx", "import {style} from 'stillwind/solid'; import {style as reactStyle} from 'stillwind/react'; const props = style('flex');").unwrap();
    assert!(
        parse(&mixed)
            .diagnostics
            .iter()
            .any(|error| error.code == "SHJS008")
    );
}

#[test]
fn semantic_reads_follow_local_aliases_without_unrelated_imports() {
    let source = SourceFile::new(
        FileId(1),
        "entry.tsx",
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
    let parsed = parse(&source);
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

#[test]
fn semantic_reads_preserve_binding_shadowing() {
    let source = SourceFile::new(
        FileId(1),
        "entry.tsx",
        r"
import { style } from 'stillwind/react';
import { handle } from './unrelated';
export function attach(handle: unknown) { return style(handle); }
",
    )
    .unwrap();
    assert!(parse(&source).summary.probe_imports.is_empty());
}

#[test]
fn extracts_only_an_import_bound_style_call_with_literal_spans() {
    let source = SourceFile::new(
        FileId(1),
        "Button.tsx",
        r#"import { style as s } from "stillwind/react";
const props = s("px-4 text-center", "hover:bg-accent");
function local(s: (value: string) => unknown) { s("not-a-style-call"); }"#,
    )
    .unwrap();

    let parsed = parse(&source);
    assert!(
        parsed.type_probes.is_empty(),
        "literal utilities need no checker queries"
    );

    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.module.style_calls.len(), 1);
    let Some(StyleInput::UtilityString(first)) = parsed.module.style_calls[0].inputs.first() else {
        panic!("the first input should be a static utility string");
    };
    assert_eq!(first.value, "px-4 text-center");
    assert_eq!(source.slice(first.span).unwrap(), "px-4 text-center");
}

#[test]
fn parses_named_and_scoped_ref_declarations() {
    let source = SourceFile::new(
        FileId(1),
        "Menu.tsx",
        r#"import { style } from "stillwind/react";
const anonymousScope = style.scope();
const menu = style.scope("menu");
const anonymous = style.ref();
const named = style.ref("trigger");
const scoped = style.ref(menu);
const scopedNamed = style.ref(menu, "item");
export const Menu = () => <div {...style(menu)}><button {...style(scoped)} /></div>;"#,
    )
    .unwrap();

    let parsed = parse(&source);

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.module.scope_definitions.len(), 2);
    assert_eq!(parsed.module.scope_definitions[0].label, None);
    assert_eq!(
        parsed.module.scope_definitions[1]
            .label
            .as_ref()
            .map(|label| label.value.as_str()),
        Some("menu")
    );
    assert_eq!(parsed.module.handle_definitions.len(), 4);
    assert_eq!(parsed.module.handle_definitions[0].scope, None);
    assert_eq!(
        parsed.module.handle_definitions[1]
            .label
            .as_ref()
            .map(|label| label.value.as_str()),
        Some("trigger")
    );
    assert_eq!(
        parsed.module.handle_definitions[2]
            .scope
            .as_ref()
            .map(|scope| scope.name.value.as_str()),
        Some("menu")
    );
    assert_eq!(
        parsed.module.handle_definitions[3]
            .label
            .as_ref()
            .map(|label| label.value.as_str()),
        Some("item")
    );
}

#[test]
fn summarizes_exported_assets_and_module_edges() {
    let source = SourceFile::new(
        FileId(1),
        r"src\Parent.tsx",
        r#"import { style } from "stillwind/react";
import { external as importedExternal } from "./external";
export const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");
export { trigger as menuTrigger };
export { sharedTransition as transition } from "./shared";
export const Parent = () => <div {...style(menu)}><button {...style(trigger)} /></div>;"#,
    )
    .unwrap();

    let parsed = parse(&source);

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.summary.module_path, "src/Parent.tsx");
    assert_eq!(parsed.summary.imports.len(), 1);
    assert_eq!(parsed.summary.imports[0].local, "importedExternal");
    assert!(matches!(
        parsed.summary.exports.get("menu"),
        Some(ExportedValue::Scopes(_))
    ));
    let Some(ExportedValue::Refs(alternatives)) = parsed.summary.exports.get("menuTrigger") else {
        panic!("menuTrigger should be an exported ref");
    };
    assert_eq!(alternatives.len(), 1);
    assert!(alternatives[0].permitted_scopes.is_some());
    assert_eq!(parsed.summary.re_exports[0].exported, "transition");
    assert_eq!(parsed.summary.re_exports[0].imported, "sharedTransition");
}

#[test]
fn leaves_conditional_handle_propagation_to_typescript() {
    let source = SourceFile::new(
        FileId(1),
        "src/choices.tsx",
        r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const alias = first;
export const selected = enabled ? alias : second;"#,
    )
    .unwrap();

    let parsed = parse(&source);
    assert!(!parsed.summary.exports.contains_key("selected"));
}

#[test]
fn spread_arguments_in_asset_macros_report_diagnostics() {
    for (body, code) in [
        ("const x=style.scope(...args);", "SHSCOPE001"),
        ("const x=style.ref(...args);", "SHREF001"),
        ("const x=style.ref(...args,'x');", "SHREF001"),
        (
            "const s=style.scope(); const x=style.ref(s,...args);",
            "SHREF001",
        ),
        ("const x=style.animation(...args);", "SHANIMATION001"),
        (
            "const a=style.animation({from:'flex'}); const x=style(a(...args));",
            "SHANIMATION002",
        ),
        ("const x=style.viewTransition(...args);", "SHVIEW001"),
        (
            "const r=style.ref(); const x=style(r.container(...args));",
            "SHREF002",
        ),
        (
            "const r=style.ref(); const x=style(r.at(...args,'flex'));",
            "SHREF004",
        ),
        (
            "const r=style.ref(); const x=style(r.at('width > 1px',...args));",
            "SHREF004",
        ),
    ] {
        let source = SourceFile::new(
            FileId(1),
            "spread.tsx",
            format!("import {{ style }} from 'stillwind/react'; {body}"),
        )
        .unwrap();
        let parsed = parse(&source);
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .unwrap_or_else(|| panic!("missing {code} for {body}: {:?}", parsed.diagnostics));
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
fn extracts_aria_and_data_attributes_with_value_expression_spans() {
    let source = SourceFile::new(
        FileId(1),
        "Button.tsx",
        r#"import { style } from "stillwind/react";
const props = style({ "aria-busy": loading, "data-state": state });"#,
    )
    .unwrap();

    let parsed = parse(&source);

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let Some(StyleInput::Attributes(entries)) = parsed.module.style_calls[0].inputs.first() else {
        panic!("the first input should be an attribute object");
    };
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name.value, "aria-busy");
    assert_eq!(source.slice(entries[0].value).unwrap(), "loading");
    assert_eq!(entries[1].name.value, "data-state");
    assert_eq!(source.slice(entries[1].value).unwrap(), "state");
}

#[test]
fn flattens_arrays_and_retains_element_local_selectors() {
    let source = SourceFile::new(
        FileId(1),
        "Button.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style(["px-4", loading && "opacity-60"], { "&:hover": "bg-accent" });"#,
    )
    .unwrap();

    let parsed = parse(&source);

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    assert!(matches!(
        parsed.module.style_calls[0].inputs[0],
        StyleInput::UtilityString(_)
    ));
    assert!(matches!(
        parsed.module.style_calls[0].inputs[1],
        StyleInput::ConditionalUtilityString { .. }
    ));
    let Some(StyleInput::SelectorUtilityString { selector, value }) =
        parsed.module.style_calls[0].inputs.get(2)
    else {
        panic!("the selector should be preserved in the style IR");
    };
    assert_eq!(selector.value, "&:hover");
    assert_eq!(value.value, "bg-accent");
}
