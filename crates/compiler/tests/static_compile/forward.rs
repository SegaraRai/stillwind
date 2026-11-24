//! `style.forward()` lowering in `style()` calls and CVA factory calls.

use stillwind_compiler::{CompileResult, CompilerOptions, render_transforms};
use stillwind_diagnostics::LintDiagnostic;
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

fn output(source: &SourceFile, result: &CompileResult) -> String {
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    render_transforms(source, &result.transforms).unwrap().code
}

#[test]
fn forward_merges_props_with_the_format_helper() {
    for (framework, helper) in [
        ("react", "stillwindClassNameObjectForward"),
        ("vue", "stillwindClassObjectForwardArrays"),
        ("solid", "stillwindClassObjectForward"),
        ("svelte", "stillwindClassStringForward"),
    ] {
        let (source, result) = compile(&format!(
            "import {{ style }} from 'stillwind/{framework}';\n\
             export const props = (p) => style('flex', style.forward({{ class: p.class, style: p.style }}));"
        ));
        let code = output(&source, &result);
        assert!(
            code.contains(&format!("{helper}(({{ class: p.class")),
            "{code}"
        );
        assert!(code.contains("Merge("), "{code}");
        assert!(!code.contains("style.forward"), "{code}");
    }
}

#[test]
fn cva_factory_takes_forward_as_its_second_argument() {
    let (source, result) = compile(
        r"
        import { style } from 'stillwind/react';
        const button = style.cva('rounded', { variants: { size: { small: 'p-2' } } });
        export const props = (p) => button({ size: 'small' }, style.forward(p));
    ",
    );
    let code = output(&source, &result);
    assert!(
        code.contains("button({ size: 'small' }, stillwindClassNameObjectForward(p))"),
        "{code}"
    );
}

#[test]
fn several_forwards_apply_in_argument_order_with_a_warning() {
    let (source, result) = compile(
        "import { style } from 'stillwind/react';
export const props = style(style.forward(a), style.forward(b));",
    );
    let code = output(&source, &result);
    let first = code
        .find("stillwindClassNameObjectForward((a))")
        .expect(&code);
    let second = code
        .find("stillwindClassNameObjectForward((b))")
        .expect(&code);
    assert!(first < second, "{code}");
    let findings = result
        .lint_findings
        .iter()
        .filter(|finding| matches!(finding.diagnostic, LintDiagnostic::MultipleStyleForwards))
        .count();
    assert_eq!(findings, 1, "{:?}", result.lint_findings);
}

#[test]
fn forward_rejects_unsupported_uses() {
    for (text, expected) in [
        (
            "import { style } from 'stillwind/lit';\nstyle('flex', style.forward(p));",
            "SW10006",
        ),
        (
            "import { style } from 'stillwind/react';\nconst f = style.forward(p);",
            "SW10004",
        ),
    ] {
        let (_, result) = compile(text);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|error| error.diagnostic.code().as_str() == expected),
            "{text}: {:?}",
            result.diagnostics
        );
    }
    let (_, result) =
        compile("import { style } from 'stillwind/react';\nstyle({ 'hover:': style.forward(p) });");
    assert!(
        result
            .lint_findings
            .iter()
            .any(|finding| matches!(&finding.diagnostic, LintDiagnostic::NestedMacroCall { .. })),
        "{:?}",
        result.lint_findings
    );
}
