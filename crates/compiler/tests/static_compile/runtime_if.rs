//! Experimental computed-value conditions through the public compiler API.

use stillwind_compiler::{CompileResult, CompilerOptions};
use stillwind_diagnostics::{CompileError, LintDiagnostic, UtilityFailure};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

/// Asserts that a malformed runtime condition capture lowers `candidate` as an omitted
/// invalid utility.
fn assert_invalid_capture(result: &CompileResult, candidate: &str) {
    assert_eq!(result.diagnostics, [] as [CompileError; 0], "{candidate}");
    assert!(
        result.lint_findings.iter().any(|finding| matches!(
            &finding.diagnostic,
            LintDiagnostic::UnresolvedUtility {
                classification: UtilityFailure::Invalid,
                ..
            }
        )),
        "{candidate}: {:?}",
        result.lint_findings
    );
    assert!(
        result.module_css.is_empty(),
        "{candidate}: {}",
        result.module_css
    );
}

fn compile(candidates: &str, native: bool) -> CompileResult {
    compile_arguments(&format!("{candidates:?}"), native)
}

fn compile_arguments(arguments: &str, native: bool) -> CompileResult {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        format!("import {{ style }} from 'stillwind/react'; export const x = style({arguments});"),
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import 'stillwind';").unwrap();
    Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions {
            use_css_if: native,
            ..CompilerOptions::default()
        },
    )
}

#[test]
fn runtime_if_intervals_follow_base_before_branch_publication() {
    let result = compile_arguments(
        r#"enabled && "if(--b):[height:2px]", "if(--a):[color:red]", other && "if(--c):[color:blue]""#,
        true,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let names = boundary_names(&result.module_css);
    assert_eq!(names.len(), 3, "{}", result.module_css);
    assert_eq!(names[0], names[1]);
    assert_ne!(names[1], names[2]);
}

fn boundary_names(css: &str) -> Vec<&str> {
    css.split("and (--")
        .skip(1)
        .map(|tail| tail.split(':').next().unwrap())
        .collect()
}

#[test]
fn runtime_if_independent_rules_share_barriers_but_conflicts_do_not() {
    for native in [false, true] {
        let result = compile(
            "if(--a):[color:red] if(--b):[width:1px] if(--c):[color:blue] if(--d):[height:2px]",
            native,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let names = boundary_names(&result.module_css);
        assert_eq!(names.len(), 4, "{}", result.module_css);
        assert_eq!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
        assert_eq!(names[2], names[3]);
        assert!(!result.module_css.contains("-stillwind-rule-boundary"));
        let mut sheet = StyleSheet::parse(&result.module_css, ParserOptions::default()).unwrap();
        sheet
            .minify(lightningcss::stylesheet::MinifyOptions::default())
            .unwrap();
        let minified = sheet
            .to_css(PrinterOptions {
                minify: true,
                ..PrinterOptions::default()
            })
            .unwrap()
            .code;
        let groups = boundary_names(&minified);
        assert!(
            groups.contains(&names[0]) && groups.contains(&names[2]),
            "{minified}"
        );
        assert!(groups.len() < names.len(), "{minified}");
    }
}

#[test]
fn runtime_if_authored_apply_and_variant_share_owner_and_expand_nested_rules() {
    let source = css_source(
        FileId(2),
        r"
        @import 'stillwind';
        .sample {
            @apply if(--a):[color:red];
            @variant if(--b) { width: 1px; }
            @variant if(--c) { @apply [color:blue]; @media print { height: 2px; } }
        }
        .sample { @variant if(--a) { color: red; } }
    ",
    )
    .unwrap();
    for native in [false, true] {
        let css = Compiler::render_stylesheet(
            &source,
            &CssSourceName::from("test.css"),
            "",
            &CompilerOptions {
                use_css_if: native,
                ..CompilerOptions::default()
            },
        )
        .unwrap();
        let names = boundary_names(&css);
        assert_eq!(names.len(), 5, "{css}");
        assert_eq!(names[0], names[1], "{css}");
        assert_ne!(names[1], names[2], "{css}");
        assert_eq!(names[2], names[3], "{css}");
        assert_ne!(names[0], names[4], "{css}");
        assert!(!css.contains("-stillwind-rule-boundary"), "{css}");
        StyleSheet::parse(&css, ParserOptions::default()).unwrap();
    }
}

#[test]
fn runtime_if_variant_name_is_owned_by_css_registration() {
    let compile_registered =
        |candidate: &str, definitions: &str| {
            let component = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; export const x = style({candidate:?});"
        )).unwrap();
            let css = SourceFile::new(FileId(2), None, None, format!(
            "@stillwind-slot utilities; @stillwind-utility flex {{display:flex;}} {definitions}"
        )).unwrap();
            Compiler::compile_source(
                &OxcSourceAnalyzer,
                &component,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &css,
                &CssSourceName::from("stillwind.css"),
                &CompilerOptions {
                    use_css_if: true,
                    ..CompilerOptions::default()
                },
            )
        };
    let absent = compile_registered("if(--c):flex", "");
    assert!(absent.module_css.is_empty(), "{}", absent.module_css);
    let registration = r"@stillwind-variant when\(*\) (value: [if-condition]) { @stillwind-intrinsic builtin-if-variant; }";
    let alias = compile_registered("when(--c:v):flex", registration);
    assert!(alias.diagnostics.is_empty(), "{:?}", alias.diagnostics);
    assert!(
        alias
            .module_css
            .contains("if(style(--c:v):flex;else:revert-rule)"),
        "{}",
        alias.module_css
    );
    assert_eq!(
        compile_registered("if(--c):flex", registration).module_css,
        ""
    );
    assert_invalid_capture(
        &compile_registered("when(!!--c):flex", registration),
        "when(!!--c):flex",
    );
    for body in [
        "@stillwind-intrinsic unknown;",
        "@stillwind-intrinsic builtin-if-variant; color:red;",
    ] {
        let css = format!(r"@stillwind-variant when\(*\) (value: [if-condition]) {{ {body} }}");
        assert_eq!(compile_registered("when(--c):flex", &css).module_css, "");
    }
    let value = compile_registered("[width:--if(--c,1px,2px)]", "");
    assert!(value.diagnostics.is_empty(), "{:?}", value.diagnostics);
    assert!(value.module_css.contains("if(style(--c):1px;else:2px)"));
}

#[test]
fn runtime_if_values_and_variants_share_the_output_option() {
    for native in [false, true] {
        let result = compile(
            "grid if(--enabled):flex w-[--if(--wide,20px,10px)] hover:if(--enabled):opacity-50!",
            native,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let css = &result.module_css;
        assert!(!css.contains("--if("), "{css}");
        assert!(css.contains("display:grid;"), "{css}");
        assert!(css.contains("revert-rule)"), "{css}");
        assert!(css.contains(")!important"), "{css}");
        if native {
            assert!(
                css.contains("display:if(style(--enabled):flex;else:revert-rule)"),
                "{css}"
            );
            assert!(
                css.contains("width:if(style(--wide):20px;else:10px)"),
                "{css}"
            );
        } else {
            assert!(css.contains(":var(--enabled) flex;"), "{css}");
            assert!(css.contains(":var(--wide) 20px;"), "{css}");
        }
        assert!(
            !css.contains("display:grid;display:"),
            "base and guard must be separate rules: {css}"
        );
    }
}

#[test]
fn runtime_if_nested_conditions_and_typed_overloads() {
    for native in [false, true] {
        let result = compile(
            "if(--a):if(--b):flex bg-[--if(--a,red,blue)] w-[--if(--a,--if(--b,1px,2px),3px)]",
            native,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(
            result.module_css.contains("background-color:"),
            "{}",
            result.module_css
        );
        assert!(!result.module_css.contains("--if("));
        assert!(
            result.module_css.contains("var(--a) var(--b)")
                || result.module_css.contains("var(--b) var(--a)"),
            "{}",
            result.module_css
        );
    }
}

#[test]
fn runtime_if_rejects_invalid_conditions_and_fallback_css_wide_true_values() {
    assert_invalid_capture(&compile("if(enabled):flex", false), "if(enabled):flex");
    for candidate in [
        "[display:--if(--a,flex)]",
        "[display:--if(--a,inherit,grid)]",
    ] {
        let result = compile(candidate, false);
        assert!(
            !result.diagnostics.is_empty(),
            "{candidate}: {}",
            result.css
        );
        assert!(result.css.is_empty() && result.transforms.is_empty());
    }
    let result = compile("[display:--if(--a,inherit,grid)]", true);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn runtime_if_negation_swaps_values_and_rejects_double_negation() {
    for native in [false, true] {
        let result = compile("w-[--if(!--c,20px,10px)]", native);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(
            if native {
                result.module_css.contains("if(style(--c):10px;else:20px)")
            } else {
                result.module_css.contains(":var(--c) 10px;")
            },
            "{}",
            result.module_css
        );
        assert_invalid_capture(&compile("if(!!--c):flex", native), "if(!!--c):flex");
        let result = compile("[width:--if(!!--c,20px,10px)]", native);
        assert_ne!(result.diagnostics, [] as [CompileError; 0]);
        assert!(result.css.is_empty() && result.transforms.is_empty());
    }
}

#[test]
fn runtime_if_negated_variants_require_native_lowering() {
    for candidate in [
        "if(!--c):flex",
        "if(!--c):if(--d):flex",
        "if(--d):if(!--c):flex",
        "if(!--c):if(!--d):flex",
    ] {
        let result = compile(candidate, true);
        assert!(
            result.diagnostics.is_empty(),
            "{candidate}: {:?}",
            result.diagnostics
        );
        assert!(
            !result.module_css.contains("var(!"),
            "{}",
            result.module_css
        );
        let fallback = compile(candidate, false);
        assert!(
            fallback
                .diagnostics
                .iter()
                .any(|error| error.diagnostic.to_string().contains("useCssIf: true")),
            "{candidate}: {:?}",
            fallback.diagnostics
        );
        assert!(fallback.css.is_empty() && fallback.transforms.is_empty());
    }
}

#[test]
fn runtime_if_value_comparisons_require_native_and_preserve_type_hints() {
    for candidate in [
        "if(--mode:compact):flex",
        "w-[--if(--mode:compact,20px,10px)]",
        "bg-[color:--if(--mode:compact,red,blue)]",
        "if(--mode:compact):if(--enabled):flex",
        "if(--enabled):if(--mode:compact):flex",
        "if(!--mode:compact):flex",
        "[width:--if(--color:rgb(1,2,3),20px,10px)]",
    ] {
        let result = compile(candidate, true);
        assert!(
            result.diagnostics.is_empty(),
            "{candidate}: {:?}",
            result.diagnostics
        );
        assert!(
            result.module_css.contains("style(--"),
            "{}",
            result.module_css
        );
        assert!(
            !result.module_css.contains("var(--mode:"),
            "{}",
            result.module_css
        );
        assert!(!result.module_css.contains("--if("));
        let fallback = compile(candidate, false);
        assert!(
            fallback.diagnostics.iter().any(|error| error
                .diagnostic
                .to_string()
                .contains("value comparisons require useCssIf: true")),
            "{candidate}: {:?}",
            fallback.diagnostics
        );
        assert!(fallback.css.is_empty() && fallback.transforms.is_empty());
    }
    let result = compile("w-[--if(--mode:compact,20px,10px)]", true);
    assert!(
        result
            .module_css
            .contains("width:if(style(--mode:compact):20px;else:10px)"),
        "{}",
        result.module_css
    );
    for candidate in ["if(--mode:):flex", "if(!!--mode:compact):flex"] {
        assert_invalid_capture(&compile(candidate, true), candidate);
    }
    assert_ne!(
        compile("[width:--if(--mode:,20px,10px)]", true).diagnostics,
        [] as [CompileError; 0]
    );
}

#[test]
fn runtime_if_authored_css_and_prefixes_use_the_same_strategy() {
    let source = css_source(
        FileId(2),
        r"
        @import 'stillwind';
        .sample { display:grid; @variant if(--enabled) { display:flex; } }
        .applied { @apply grid if(--enabled):flex; }
        .value { width:--if(--enabled,20px,10px); --sw-user:1; color:var(--sw-user); }
    ",
    )
    .unwrap();
    for native in [false, true] {
        let options = CompilerOptions {
            use_css_if: native,
            prefix: "--test-".into(),
            ..CompilerOptions::default()
        };
        let css = Compiler::render_stylesheet(
            &source,
            &CssSourceName::from("stillwind.css"),
            "",
            &options,
        )
        .unwrap();
        assert!(
            !css.contains("--if(") && !css.contains("@variant") && !css.contains("@apply"),
            "{css}"
        );
        assert!(css.contains("@supports (x:revert-rule) and (--"), "{css}");
        assert!(!css.contains("--sw-if"), "{css}");
        assert!(
            css.contains("--sw-user:1") && css.contains("var(--sw-user)"),
            "{css}"
        );
        assert_eq!(css.contains("if(style(--enabled)"), native);
    }
}
