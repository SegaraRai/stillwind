//! `or(...)` variants in utility candidates, wrapper keys, and definitions.
//!
//! Pins that an `or(...)` variant means the utilities that its sequences expand to, in
//! every place that accepts variants, and that a malformed or failing alternative omits
//! the whole candidate.

use stillwind_compiler::{CompileResult, CompilerOptions};
use stillwind_diagnostics::LintKind;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

fn compile(body: &str) -> CompileResult {
    let text = format!(
        "import {{ style }} from 'stillwind/react'; const card = style.ref('card'); {body}"
    );
    let source = SourceFile::new(FileId(1), None, None, text.as_str()).unwrap();
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
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result
}

/// Compiles `body` and returns the stylesheet with numbered class names.
fn stylesheet(body: &str) -> String {
    let result = compile(body);
    assert!(
        result.lint_findings.is_empty(),
        "{body}: {:#?}",
        result.lint_findings
    );
    let mut css = result.css.clone();
    for (index, class) in result.classes.iter().enumerate() {
        css = css.replace(class.class_name.as_str(), &format!("test_class_{index}"));
    }
    css
}

#[test]
fn alternatives_mean_the_utilities_of_each_sequence() {
    for (alternatives, expanded) in [
        (
            r#"export const a = style("or(hover,md:focus):p-4 p-2");"#,
            r#"export const a = style("hover:p-4 md:focus:p-4 p-2");"#,
        ),
        (
            r#"export const a = style("md:or(hover,[&.open]):or(*,dark):underline!");"#,
            r#"export const a = style("md:hover:*:underline! md:hover:dark:underline! md:[&.open]:*:underline! md:[&.open]:dark:underline!");"#,
        ),
        (
            r#"export const a = style({ "or(hover,focus):": { "md:": "p-4" } });"#,
            r#"export const a = style("hover:md:p-4 focus:md:p-4");"#,
        ),
        (
            r#"export const a = style({ [card("or(hover,focus-within)")]: "p-4" });"#,
            r#"export const a = style({ [card("hover")]: "p-4", [card("focus-within")]: "p-4" });"#,
        ),
        (
            r#"const s = style.when("hover", card("focus")); export const a = style({ "md:": { [s]: "p-4" } }, `${s}:m-2`);"#,
            r#"export const a = style({ "md:": { "hover:": "p-4", [card("focus")]: "p-4" } }, "hover:m-2", `${card("focus")}:m-2`);"#,
        ),
        (
            r#"export const t = style.viewTransition({ old: "or(md,print):opacity-0" }); export const a = style(t.for("pair"));"#,
            r#"export const t = style.viewTransition({ old: "md:opacity-0 print:opacity-0" }); export const a = style(t.for("pair"));"#,
        ),
    ] {
        assert_eq!(
            stylesheet(alternatives),
            stylesheet(expanded),
            "{alternatives}"
        );
    }
}

#[test]
fn css_calls_expand_alternatives_in_candidates_and_keys() {
    let alternatives = compile(
        r#"export const sheet = style.css({ "[button]:": ["or(hover,focus):p-4", { "or(md,print):": "m-1" }] });"#,
    );
    let expanded = compile(
        r#"export const sheet = style.css({ "[button]:": ["hover:p-4 focus:p-4", { "md:": "m-1", "print:": "m-1" }] });"#,
    );
    assert!(
        alternatives.lint_findings.is_empty(),
        "{:#?}",
        alternatives.lint_findings
    );
    let replacements = |result: &CompileResult| {
        result
            .transforms
            .iter()
            .map(|transform| transform.replacement.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(replacements(&alternatives), replacements(&expanded));
}

#[test]
fn a_failing_alternative_omits_the_candidate() {
    for (candidate, kind) in [
        ("or():p-4", LintKind::UnknownVariant),
        ("not-or(hover,focus):p-4", LintKind::InvalidVariant),
        ("or(hover,):p-4", LintKind::InvalidVariant),
        ("or(hover,unknown-name):p-4", LintKind::UnknownVariant),
        (
            "or(a,b):or(a,b):or(a,b):or(a,b):or(a,b):or(a,b):p-4",
            LintKind::InvalidVariant,
        ),
    ] {
        let result = compile(&format!("export const a = style(\"{candidate}\");"));
        let kinds = result
            .lint_findings
            .iter()
            .map(stillwind_diagnostics::LintFinding::kind)
            .collect::<Vec<_>>();
        assert_eq!(kinds, [kind], "{candidate}");
        assert!(
            !result.css.contains("calc(var(--spacing) * 4)"),
            "{candidate}: {}",
            result.css
        );
    }
}
