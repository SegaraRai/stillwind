//! `style.media()` lowering through the public compiler API.

use stillwind_compiler::{CompileResult, CompilerOptions, render_transforms};
use stillwind_diagnostics::{LintDiagnostic, LintOverrides, LintSeverity};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

fn compile(text: &str, options: &CompilerOptions) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(FileId(1), None, None, text).unwrap();
    let theme = css_source(
        FileId(2),
        "@import 'stillwind'; @custom-variant narrow (@media (width < 30rem));",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        options,
    );
    (source, result)
}

fn output(text: &str) -> (String, CompileResult) {
    let (source, result) = compile(text, &CompilerOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let code = render_transforms(&source, &result.transforms).unwrap().code;
    (code, result)
}

#[test]
fn calls_become_query_strings_and_release_the_macro_import() {
    let (code, result) = output(
        r#"
        import { style as sw } from 'stillwind/react';
        export const wide = sw.media("sm:motion-reduce", `not-md:`);
        export function query() {
            return window.matchMedia(sw.media('narrow:'));
        }
        "#,
    );
    assert!(
        code.contains(
            r#"export const wide = "(width >= 40rem) and (prefers-reduced-motion: reduce), not (width >= 48rem)";"#
        ),
        "{code}"
    );
    assert!(
        code.contains(r#"window.matchMedia("(width < 30rem)")"#),
        "{code}"
    );
    assert!(!code.contains("stillwind/react"), "{code}");
    assert!(result.module_css.is_empty(), "{}", result.module_css);
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
}

#[test]
fn media_queries_and_style_calls_lower_in_one_module() {
    let (code, _) = output(
        r"
        import { style } from 'stillwind/react';
        const query = style.media('lg');
        export const props = style('lg:p-4');
        export { query };
        ",
    );
    assert!(
        code.contains(r#"const query = "(width >= 64rem)";"#),
        "{code}"
    );
    assert!(!code.contains("style("), "{code}");
}

#[test]
fn invalid_arguments_match_nothing_at_every_reporting_level() {
    let source = r"
        import { style } from 'stillwind/react';
        export const a = style.media('sm:hover', 'future:', name, ...rest);
        export const b = style.media('sm:future:', 'print');
        ";
    for severity in [LintSeverity::Deny, LintSeverity::Warn, LintSeverity::Off] {
        let options = CompilerOptions {
            lint: LintOverrides {
                invalid_media_query: Some(severity),
                unknown_variant: Some(severity),
                ..LintOverrides::default()
            },
            ..CompilerOptions::default()
        };
        let (source, result) = compile(source, &options);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let code = render_transforms(&source, &result.transforms).unwrap().code;
        assert!(code.contains(r#"export const a = "not all";"#), "{code}");
        assert!(code.contains(r#"export const b = "print";"#), "{code}");
        let mut kinds = result
            .lint_findings
            .iter()
            .map(|finding| match &finding.diagnostic {
                LintDiagnostic::NonMediaVariant { variant } => format!("media:{variant}"),
                LintDiagnostic::InvalidMediaArgument { .. } => "argument".into(),
                LintDiagnostic::UnknownVariant { variant, .. } => format!("unknown:{variant}"),
                other => panic!("unexpected finding {other:?}"),
            })
            .collect::<Vec<_>>();
        kinds.sort();
        let expected = [
            "argument",
            "argument",
            "media:hover",
            "unknown:future",
            "unknown:future",
        ];
        if severity != LintSeverity::Off {
            assert_eq!(kinds, expected, "{severity:?}");
        }
    }
}

/// A call in the arguments of another macro call omits the smallest input that would
/// keep it as JavaScript, and the other inputs keep their output.
#[test]
fn calls_inside_other_macro_calls_omit_the_input_that_contains_them() {
    for (input, expected) in [
        (
            "{ '--query': style.media('sm'), 'data-state': 'open' }",
            "{ 'data-state': 'open' }",
        ),
        ("style.media('sm'), 'p-4'", "'p-4'"),
        ("style.media('sm') ? 'p-4' : 'm-4', 'block'", "'block'"),
        ("matches(style.media('sm')) && 'p-4', 'block'", "'block'"),
        ("{ class: [style.media('sm'), name] }, 'block'", "'block'"),
        ("{ [style.media('sm')]: 'p-4' }, 'block'", "'block'"),
        ("{ 'md:': style.media('sm') }, 'block'", "'block'"),
    ] {
        let compile_body = |body: &str| {
            compile(
                &format!(
                    "import {{ style }} from 'stillwind/react'; export const props = style({body});"
                ),
                &CompilerOptions::default(),
            )
        };
        let (source, result) = compile_body(input);
        assert!(
            result.diagnostics.is_empty(),
            "{input}: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result
                .lint_findings
                .iter()
                .filter(|finding| matches!(&finding.diagnostic, LintDiagnostic::NestedMacroCall { syntax } if syntax == "style.media()"))
                .count(),
            1,
            "{input}: {:?}",
            result.lint_findings
        );
        let code = render_transforms(&source, &result.transforms).unwrap().code;
        assert!(!code.contains("style"), "{input}: {code}");
        let (_, expected) = compile_body(expected);
        assert_eq!(result.module_css, expected.module_css, "{input}");
    }

    let (source, result) = compile(
        r"
        import { style } from 'stillwind/react';
        export const choose = style.cva({ '': 'p-4', '--query': style.media('sm') });
        ",
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let code = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(!code.contains("style.media"), "{code}");
    assert!(
        result
            .lint_findings
            .iter()
            .any(|finding| matches!(&finding.diagnostic, LintDiagnostic::NestedMacroCall { syntax } if syntax == "style.media()")),
        "{:?}",
        result.lint_findings
    );
}
