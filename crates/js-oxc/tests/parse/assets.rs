//! Checks asset declarations, uses, and exported identities.

use stillwind_diagnostics::LintDiagnostic;

use super::*;

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
        assert_eq!(value.kind(), AssetKind::Ref);
        let candidates = value.finite_candidates().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates.first().unwrap().binding_name().as_str(), binding);
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
            LintDiagnostic::InvalidRefArgument,
        ),
        (
            "const x=style.keyframes(...args);",
            LintDiagnostic::InvalidKeyframesDefinition,
        ),
        (
            "const a=style.keyframes({from:'flex'}); const x=style(a(...args));",
            LintDiagnostic::InvalidKeyframesOptions,
        ),
        (
            "const x=style.viewTransition(...args);",
            LintDiagnostic::InvalidViewTransitionDefinition,
        ),
        (
            "const r=style.ref(); const x=style(r.asContainer(...args));",
            LintDiagnostic::InvalidContainerType,
        ),
        (
            "const r=style.ref(); const x=style({[r.at(...args)]: 'flex'});",
            LintDiagnostic::NonStaticRefVariant,
        ),
        (
            "const r=style.ref(); const x=style({[r.at('width > 1px',...args)]: 'flex'});",
            LintDiagnostic::NonStaticRefVariant,
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
            .lint_findings
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
        ("label", LintDiagnostic::InvalidRefArgument),
        ("42", LintDiagnostic::InvalidRefArgument),
        ("`item`", LintDiagnostic::InvalidRefArgument),
        ("'one', 'two'", LintDiagnostic::InvalidRefArguments),
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
                .lint_findings
                .iter()
                .any(|error| error.diagnostic == expected),
            "{arguments}: {:?}",
            parsed.diagnostics
        );
        assert!(parsed.diagnostics.is_empty(), "{arguments}: {parsed:#?}");
        assert_eq!(parsed.module.ref_definitions.len(), 1, "{arguments}");
        assert!(
            parsed.module.ref_definitions[0].label.is_none(),
            "{arguments}"
        );
    }
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
            parsed.diagnostics.iter().any(|error| error.diagnostic
                == CompileDiagnostic::UnconsumedMacroUse {
                    syntax: "keyframes use".into()
                }),
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
