//! `style.css()` lowering through the public compiler API.

use stillwind_compiler::{CompileResult, CompilerOptions, render_transforms};
use stillwind_css::strip_css_comments;
use stillwind_diagnostics::{LintDiagnostic, LintKind};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

/// Compiles `body` after a React macro import against the standard package followed by
/// `css`, and returns the stylesheet string of `export const sheet = ...`.
fn compile(body: &str, css: &str) -> (String, CompileResult) {
    let text = format!("import {{ style }} from 'stillwind/react';\n{body}");
    let source = SourceFile::new(FileId(1), None, None, text.as_str()).unwrap();
    let theme = css_source(FileId(2), format!("@import 'stillwind'; {css}")).unwrap();
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
    let code = render_transforms(&source, &result.transforms).unwrap().code;
    assert!(!code.contains("stillwind/react"), "{code}");
    assert!(result.module_css.is_empty(), "{}", result.module_css);
    let literal = code
        .split_once("export const sheet = ")
        .and_then(|(_, rest)| rest.split_once(";\n").or_else(|| rest.rsplit_once(';')))
        .map_or_else(|| panic!("{code}"), |(literal, _)| literal);
    (serde_json::from_str(literal).unwrap(), result)
}

fn kinds(result: &CompileResult) -> Vec<LintKind> {
    result
        .lint_findings
        .iter()
        .map(stillwind_diagnostics::LintFinding::kind)
        .collect()
}

#[test]
fn the_default_template_places_used_support_before_the_rules() {
    let (css, result) = compile(
        r#"export const sheet = style.css("host:block [:host]:p-4", { "[button]:": ["hover:bg-red-500", { "": "translate-x-2" }] });"#,
        "",
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
    assert!(css.starts_with("/*! stillwind v"));
    assert!(css.contains("/*! tailwindcss v"));
    let theme = css.find(":where(:root, :host){").unwrap();
    let initials = css.find("{--tw-translate-x:0;").unwrap();
    let rules = css.find(":host{display:block;}").unwrap();
    assert!(theme < initials && initials < rules, "{css}");
    for expected in [
        "--color-red-500:",
        "--spacing:0.25rem;",
        ":host{padding:calc(var(--spacing) * 4);}",
        "button:hover{background-color:var(--color-red-500);}",
        "button{--tw-translate-x:calc(var(--spacing) * 2);",
    ] {
        assert!(css.contains(expected), "{expected}: {css}");
    }
    // Unused tokens, preflight, and registrations stay out of the string.
    for unexpected in ["--color-blue-500", "box-sizing", "@property", "@layer"] {
        assert!(!css.contains(unexpected), "{unexpected}: {css}");
    }
}

#[test]
fn the_selector_option_targets_utilities_that_select_no_element() {
    let (css, result) = compile(
        r#"export const sheet = style.css("p-4 animate-spin [button]:m-2", { selector: ":host([size=m])" });"#,
        "",
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
    assert!(css.contains(":host([size=m]){padding:"), "{css}");
    assert!(css.contains("button{margin:"), "{css}");
    assert!(css.contains("@keyframes spin"), "{css}");

    let (css, result) = compile(r#"export const sheet = style.css("p-4 [button]:m-2");"#, "");
    assert_eq!(kinds(&result), [LintKind::MissingDocumentTarget]);
    assert_eq!(
        strip_css_comments(&css).trim(),
        ":where(:root, :host){--spacing:0.25rem;}button{margin:calc(var(--spacing) * 2);}"
    );
}

#[test]
fn named_templates_choose_placement_and_omit_missing_slots() {
    let templates = "\
        @stillwind-css-template default { @stillwind-slot utilities; } \
        @stillwind-css-template shadow { \
          @layer theme { :host { @stillwind-slot theme; } } \
          @layer utilities { @stillwind-slot utilities; } \
          .note { color: gray; } \
        }";
    let (css, _) = compile(r#"export const sheet = style.css("host:p-4");"#, templates);
    assert_eq!(
        strip_css_comments(&css).trim(),
        ":host{padding:calc(var(--spacing) * 4);}"
    );
    let (css, _) = compile(
        r#"export const sheet = style.css("host:p-4", { template: "shadow" });"#,
        templates,
    );
    assert!(
        strip_css_comments(&css)
            .trim_start()
            .starts_with("@layer theme{:host{--spacing:0.25rem;}}@layer utilities{:host{padding:"),
        "{css}"
    );
    assert!(css.ends_with(".note { color: gray; }"), "{css}");
}

#[test]
fn unsupported_inputs_and_options_are_omitted_with_findings() {
    let (css, result) = compile(
        r#"export const sheet = style.css("host:block", flag && "host:p-4", { "unknown": "open" }, { "--size": "1px" }, ...rest);"#,
        "",
    );
    assert!(css.ends_with(":host{display:block;}"), "{css}");
    assert!(!css.contains("padding"), "{css}");
    assert_eq!(kinds(&result), [LintKind::InvalidCssInput; 4]);

    let (css, result) = compile(
        r#"export const sheet = style.css("host:block", { template: "missing" });"#,
        "",
    );
    assert_eq!(strip_css_comments(&css).trim(), ":host{display:block;}");
    assert!(matches!(
        &result.lint_findings[..],
        [finding] if matches!(&finding.diagnostic, LintDiagnostic::InvalidCssOption { reason } if reason.contains("`missing`"))
    ));

    let (css, result) = compile(
        r#"export const sheet = style.css("host:block", { "foo:": "host:p-4" });"#,
        "",
    );
    assert!(!css.contains("padding"), "{css}");
    assert_eq!(kinds(&result), [LintKind::UnknownVariant]);
}

#[test]
fn calls_inside_style_inputs_omit_the_input() {
    let text = "import { style } from 'stillwind/react'; export const props = style({ '--sheet': style.css('host:block'), 'unknown': 'open' });";
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
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.lint_findings.iter().any(|finding| matches!(
        &finding.diagnostic,
        LintDiagnostic::NestedMacroCall { syntax } if syntax == "style.css()"
    )));
}

#[test]
fn conditions_on_the_shadow_host_move_into_the_functional_pseudo_class() {
    let (css, result) = compile(
        r#"export const sheet = style.css("host:hover:block host:hover:before:flex [:host(.a)]:focus:grid host:*:p-4");"#,
        "",
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
    for selector in [
        ":host(:hover){display:block;}",
        ":host(:hover)::before{",
        ":host(.a:focus){display:grid;}",
        ":host>*",
    ] {
        assert!(
            css.replace(" > ", ">").contains(selector),
            "{selector}: {css}"
        );
    }
    assert!(!css.contains(":host:hover"), "{css}");

    let (css, _) = compile(
        r#"export const sheet = style.css("hover:block", { selector: ":host(.a)" });"#,
        "",
    );
    assert!(css.contains(":host(.a:hover){display:block;}"), "{css}");
}

#[test]
fn selectors_that_read_outside_the_shadow_tree_are_reported() {
    for (input, reported) in [
        ("host:in-[.page]:block", true),
        ("host:group-hover:block", true),
        ("host:hover:block", false),
        ("host:has-[.icon]:block", false),
    ] {
        let (_, result) = compile(
            &format!(r#"export const sheet = style.css("{input}");"#),
            "",
        );
        assert_eq!(
            kinds(&result).contains(&LintKind::UnmatchedHostSelector),
            reported,
            "{input}: {:?}",
            result.lint_findings
        );
    }
}

#[test]
fn conditional_rules_receive_their_support_queries() {
    let (css, result) = compile(
        r#"export const sheet = style.css("grid if(--on):flex hover:if(--on):block", { selector: ":host" });"#,
        "",
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
    assert!(!css.contains("-stillwind-rule-boundary"), "{css}");
    assert!(css.contains("@supports (x:revert-rule) and (--"), "{css}");
    assert!(css.contains(":host{display:grid;}"), "{css}");
}
