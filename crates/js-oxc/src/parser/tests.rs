//! Integration of import binding, declarations, reference resolution and module summaries.
use super::entry::parse;
use stillwind_ir::StyleInput;
use stillwind_semantic::ExportedValue;
use stillwind_source::{FileId, SourceFile};

#[test]
fn extracts_only_an_import_bound_style_call_with_literal_spans() {
    let source = SourceFile::new(
        FileId(1),
        "Button.tsx",
        r#"import { style as s } from "stillwind/react";
const props = s("px-4 text-center", "hover:bg-accent");
function local(s: (value: string) => unknown) { s("not-a-style-call"); }"#,
    );

    let parsed = parse(&source);

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
    );

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
    );

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
    );

    let parsed = parse(&source);
    assert!(!parsed.summary.exports.contains_key("selected"));
}
