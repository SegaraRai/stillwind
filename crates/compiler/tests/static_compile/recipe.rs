//! Recipe extraction, style lowering, and runtime imports through the public compiler API.

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
fn recipe_compiles_values_and_preserves_selection_configuration() {
    let (source, result) = compile(
        r"
        import { style as sw } from 'stillwind/react';
        export const choose = sw.recipe({ '': 'rounded' }, {
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
    assert!(
        output.contains("stillwindClassNameObjectRecipe"),
        "{output}"
    );
    assert!(output.contains("from \"stillwind/runtime\""), "{output}");
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
    assert!(
        result.module_css.contains(":hover") && result.module_css.contains("box-shadow"),
        "{}",
        result.module_css
    );
}

#[test]
fn recipe_keeps_literal_and_dynamic_custom_properties_in_fragments() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/svelte';
        export const choose = style.recipe(['block', { '--tone': tone, '--contrast': enabled ? 'black' : 'white' }], {
            variants: {
                kind: {
                    plain: true,
                    fixed: ['text-[var(--tone)]', { '--tone': 'red' }],
                    hovered: { 'hover:': ['p-2', true] },
                },
            },
        });
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(
        output.contains("stillwindClassStringCustomPropertiesKey"),
        "{output}"
    );
    assert!(output.contains("stillwindCssValue(tone)"), "{output}");
    assert!(output.contains("\"red\""), "{output}");
    assert!(output.contains("enabled ? 'black' : 'white'"), "{output}");
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
fn recipe_rejects_javascript_conditionals_in_utility_definitions() {
    for value in [
        "enabled && 'p-2'",
        "enabled ? 'p-2' : 'p-3'",
        "['block', enabled && 'p-2']",
        "['block', enabled ? 'p-2' : 'p-3']",
        "{'hover:': enabled && 'p-2'}",
        "{'hover:': [{'focus:': enabled ? 'p-2' : 'p-3'}]}",
    ] {
        for body in [
            format!(
                "style.recipe({value}, {{ variants: {{ size: {{ small: {{'hover:': 'block'}} }} }} }})"
            ),
            format!(
                "style.recipe({{'hover:': 'block'}}, {{ variants: {{ size: {{ small: {value} }} }} }})"
            ),
            format!(
                "style.recipe({{'hover:': 'block'}}, {{ variants: {{ size: {{ small: 'text-sm' }} }}, \
                 compoundVariants: [{{ size: 'small', class: {value} }}] }})"
            ),
            format!(
                "style.recipe({{'hover:': 'block'}}, {{ variants: {{ size: {{ small: 'text-sm' }} }}, \
                 compoundVariants: [{{ size: 'small', className: {value} }}] }})"
            ),
        ] {
            let (source, result) = compile(&format!(
                "import {{ style }} from 'stillwind/react'; const choose = {body};"
            ));
            assert!(result.diagnostics.is_empty(), "{body}: {result:#?}");
            assert!(
                result
                    .lint_findings
                    .iter()
                    .any(|finding| finding.diagnostic == LintDiagnostic::InvalidRecipeInput),
                "{body}: {result:#?}"
            );
            assert!(
                result.module_css.contains(":hover") && result.module_css.contains("display:block"),
                "{body}: {}",
                result.module_css
            );
            assert!(
                !result.module_css.contains("padding:"),
                "{body}: {}",
                result.module_css
            );
            let output = render_transforms(&source, &result.transforms).unwrap().code;
            assert!(!output.contains("enabled"), "{body}: {output}");
        }
    }
}

#[test]
fn recipe_omits_unsupported_values_and_preserves_valid_configuration() {
    for body in [
        "style.recipe(null, config)",
        "style.recipe(null, { variants: { size: { small: 'p-2' } }, defaultVariants: { size: 'large' } })",
        "style.recipe(null, { variants: { className: { a: 'p-2' } } })",
        "style.recipe(null, { compoundVariants: [{ missing: 'a', class: 'block' }] })",
        "style.recipe(null, { ...config })",
        "style.recipe({ 'unknown': 'ready' })",
        "style.recipe({ class: classes })",
        "style.recipe(style('block'))",
        "style.recipe(`p-${size}`)",
        "style.recipe([...styles])",
        "style?.recipe('block')",
    ] {
        let (source, result) = compile(&format!(
            "import {{ style }} from 'stillwind/react'; const choose = {body};"
        ));
        assert!(
            result.lint_findings.iter().any(|diagnostic| matches!(
                diagnostic.diagnostic,
                LintDiagnostic::InvalidRecipeConfig { .. } | LintDiagnostic::InvalidRecipeInput
            )),
            "{body}: {:?}",
            result.diagnostics
        );
        assert!(result.diagnostics.is_empty(), "{body}: {result:#?}");
        let output = render_transforms(&source, &result.transforms).unwrap().code;
        assert!(
            output.contains("stillwindClassNameObjectRecipe"),
            "{body}: {output}"
        );
        assert!(!output.contains("style.recipe"), "{body}: {output}");
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
fn recipe_omits_invalid_rule_groups_and_keeps_other_entries() {
    for key in ["hover:", "", ":"] {
        let (_, result) = compile(&format!(
            "import {{ style }} from 'stillwind/react'; \
             const choose = style.recipe({{'{key}': ['p-2', {{'--tone': tone}}], \
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
fn recipe_empty_calls_and_shadowed_members_remain_valid_javascript() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/react';
        export const empty = style.recipe();
        export const undefinedConfig = style.recipe('block', undefined);
        export function ordinary(style: { recipe: (value: string) => string }) {
            return style.recipe('external');
        }
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(
        output.contains("stillwindClassNameObjectRecipe()"),
        "{output}"
    );
    assert!(
        output.contains("return style.recipe('external')"),
        "{output}"
    );
}

#[test]
fn recipe_slots_compile_values_by_slot() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/react';
        export const card = style.recipe('rounded', {
            slots: { icon: 'size-4', label: 'text-sm' },
            variants: {
                size: { small: { base: 'p-2', icon: 'size-3' }, large: 'p-8' },
                tone: { quiet: { 'hover:': 'opacity-50' } },
            },
            compoundVariants: [{ size: 'large', class: { label: 'text-lg' } }],
            compoundSlots: [{ slots: ['icon', 'label'], size: 'small', class: 'gap-1' }],
        });
    ",
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(output.contains("slots: { icon: \"sw_"), "{output}");
    assert!(output.contains("small: { base: \"sw_"), "{output}");
    assert!(output.contains("slots: ['icon', 'label']"), "{output}");
    for declaration in ["width", "font-size", "padding", "gap", ":hover"] {
        assert!(
            result.module_css.contains(declaration),
            "{declaration}: {}",
            result.module_css
        );
    }
}

#[test]
fn recipe_slots_report_unknown_and_invalid_slots() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/react';
        export const card = style.recipe('rounded', {
            slots: { icon: 'size-4', 'hover:': 'p-1' },
            variants: { size: { small: { label: 'p-2', icon: 'size-3' } } },
            compoundSlots: [{ slots: ['label'], class: 'gap-1' }],
        });
        export const plain = style.recipe('rounded', {
            compoundSlots: [{ slots: ['base'], class: 'gap-1' }],
        });
    ",
    );
    let reasons = result
        .lint_findings
        .iter()
        .filter_map(|finding| match &finding.diagnostic {
            LintDiagnostic::InvalidRecipeConfig { reason } => Some(reason.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reasons,
        [
            "invalid slot name",
            "unknown slot",
            "slots must be a literal array of declared slot names",
            "compoundSlots requires slots",
        ],
        "{reasons:?}"
    );
    let output = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(!output.contains("gap-1"), "{output}");
    assert!(!result.module_css.contains("gap"), "{}", result.module_css);
}
