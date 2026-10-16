//! CVA extraction, style lowering, and runtime imports through the public compiler API.

use stillwind_compiler::{CompileResult, CompilerOptions, render_transforms};
use stillwind_diagnostics::{LintDiagnostic, RuleGroupInputErrorKind};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

fn compile(text: &str) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(FileId(1), None, None, text).unwrap();
    let theme = css_source(FileId(2), "@import 'stillwind';").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    (source, result)
}

#[test]
fn cva_compiles_values_and_preserves_selection_configuration() {
    let (source, result) = compile(
        r"
        import { style as sw } from 'stillwind/react';
        export const choose = sw.cva({ '': 'rounded' }, {
            variants: {
                size: { small: { ':': 'p-4 px-2' }, large: 'p-8 py-2' },
                enabled: { false: null, true: 'block' },
                count: { 0: 'm-0', 1: 'm-1' },
            },
            defaultVariants: { size: 'small', enabled: false, count: 0 },
            compoundVariants: [{ size: ['small', 'large'], class: { 'hover:': { '': 'shadow-md' } } }],
        });
        export const props = choose({ size: value });
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(output.contains("stillwindCva"), "{output}");
    assert!(output.contains("stillwind/runtime/react"), "{output}");
    assert!(
        output.contains("defaultVariants: { size: 'small', enabled: false, count: 0 }"),
        "{output}"
    );
    assert!(output.contains("choose({ size: value })"), "{output}");
    assert!(
        result.module_css.contains("padding"),
        "{}",
        result.module_css
    );
}

#[test]
fn cva_keeps_literal_and_dynamic_custom_properties_in_fragments() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/svelte';
        export const choose = style.cva(['block', { '--tone': tone }], {
            variants: {
                kind: {
                    plain: true,
                    fixed: ['text-[var(--tone)]', { '--tone': 'red' }],
                    guarded: enabled && { 'hover:': ['p-2', true] },
                },
            },
        });
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(output.contains("stillwindCustomPropertiesKey"), "{output}");
    assert!(output.contains("stillwindCssValue(tone)"), "{output}");
    assert!(output.contains("\"red\""), "{output}");
    assert!(output.contains("enabled"), "{output}");
    assert!(
        result.module_css.contains("var(--tone)"),
        "{}",
        result.module_css
    );
    assert!(
        result
            .transforms
            .iter()
            .flat_map(|transform| &transform.expressions)
            .any(|mapping| {
                &source.text()[mapping.original.start.0 as usize..mapping.original.end.0 as usize]
                    == "tone"
            })
    );
}

#[test]
fn cva_omits_unsupported_values_and_preserves_valid_configuration() {
    for body in [
        "style.cva(null, config)",
        "style.cva(null, { variants: { size: { small: 'p-2' } }, defaultVariants: { size: 'large' } })",
        "style.cva(null, { variants: { className: { a: 'p-2' } } })",
        "style.cva(null, { compoundVariants: [{ missing: 'a', class: 'block' }] })",
        "style.cva(null, { ...config })",
        "style.cva({ 'data-state': 'ready' })",
        "style.cva({ class: classes })",
        "style.cva(style('block'))",
        "style.cva(`p-${size}`)",
        "style.cva([...styles])",
        "style?.cva('block')",
    ] {
        let (source, result) = compile(&format!(
            "import {{ style }} from 'stillwind/react'; const choose = {body};"
        ));
        assert!(
            result.lint_findings.iter().any(|diagnostic| matches!(
                diagnostic.diagnostic,
                LintDiagnostic::InvalidCvaConfig { .. } | LintDiagnostic::InvalidCvaInput
            )),
            "{body}: {:?}",
            result.diagnostics
        );
        assert!(result.diagnostics.is_empty(), "{body}: {result:#?}");
        let output = render_transforms(&source, &result.transforms).unwrap().code;
        assert!(output.contains("stillwindCva"), "{body}: {output}");
        assert!(!output.contains("style.cva"), "{body}: {output}");
        if body.contains("small:") {
            assert!(
                result.module_css.contains("padding:"),
                "{body}: {}",
                result.module_css
            );
        } else {
            assert!(result.module_css.is_empty(), "{body}");
        }
    }
}

#[test]
fn cva_omits_invalid_rule_groups_and_keeps_other_entries() {
    for key in ["hover:", "", ":"] {
        let (_, result) = compile(&format!(
            "import {{ style }} from 'stillwind/react'; \
             const choose = style.cva({{'{key}': ['p-2', {{'--tone': tone}}], \
             'focus:': 'block', '--kept': 'blue'}});"
        ));
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.iter().any(|finding| finding.diagnostic
            == LintDiagnostic::InvalidRuleGroupInput {
                reason: RuleGroupInputErrorKind::CustomProperty,
            }));
        assert!(
            result.module_css.contains("display:block"),
            "{}",
            result.module_css
        );
        assert!(
            !result.module_css.contains("padding:"),
            "{}",
            result.module_css
        );
        let output = &result.transforms[0].replacement;
        assert!(output.contains("blue"), "{output}");
        assert!(!output.contains("--tone"), "{output}");
    }
}

#[test]
fn cva_empty_calls_and_shadowed_members_remain_valid_javascript() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/react';
        export const empty = style.cva();
        export const undefinedConfig = style.cva('block', undefined);
        export function ordinary(style: { cva: (value: string) => string }) {
            return style.cva('external');
        }
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(output.contains("stillwindCva()"), "{output}");
    assert!(output.contains("return style.cva('external')"), "{output}");
}
