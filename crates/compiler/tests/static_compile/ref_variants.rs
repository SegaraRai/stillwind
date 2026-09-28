//! Ref variants in selectors and templates.
//!
//! Pins that ref presence uses the existing boundary or ancestor relation, that call
//! and tag spellings normalize the same way, that direct interpolation matches ordered
//! nested wrappers, that custom alternatives and media conditions survive lowering, and
//! that an unusable ref variant reports a diagnostic instead of emitting an unscoped
//! selector.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::LintKind;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};

use super::*;

fn compile(args: &str, bounded: bool) -> (String, String) {
    let ref_definition = if bounded {
        "const menu = style.ref(\"menu\"); const trigger = style.ref(menu, \"trigger\");"
    } else {
        "const trigger = style.ref(\"trigger\");"
    };
    let use_site = if bounded {
        format!(
            "const View = () => <section {{...style(menu)}}><button {{...style(trigger)}}/><span {{...style({args})}}/></section>;"
        )
    } else {
        format!("const props = style({args});")
    };
    let source = SourceFile::new(FileId(1), None, None, format!(
        "import {{ style }} from \"stillwind/react\"; {ref_definition} const other = style.ref(\"other\"); declare const active: boolean; {use_site}"
    )).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        r#"
@import "stillwind";
@theme { --color-accent: red; }
@variant hocus { &:hover { @slot; } &:focus { @slot; } }
"#,
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
        &CompilerOptions::default(),
    );
    assert!(
        result.diagnostics.is_empty(),
        "{args}: {:#?}",
        result.diagnostics
    );
    assert!(
        result.lint_findings.is_empty(),
        "{args}: {:#?}",
        result.lint_findings
    );
    (result.css.clone(), result.apply_to_source(&source).unwrap())
}

#[test]
fn ref_presence_uses_the_existing_boundary_or_ancestor_relation() {
    for bounded in [false, true] {
        let (css, code) = compile(r#"{ [trigger()]: "text-accent" }"#, bounded);
        assert_eq!(
            compile(r"`${trigger()}:text-accent`", bounded),
            (css.clone(), code.clone())
        );
        let utilities = css.split("@layer utilities{").nth(1).unwrap();
        assert!(
            utilities.contains("color:var(--color-accent)"),
            "{utilities}"
        );
        assert!(utilities.contains(":where("), "{utilities}");
        assert!(!code.contains("trigger()"), "{code}");
        if bounded {
            assert!(css.contains(":has(:where(.sw_"), "{css}");
        } else {
            assert!(css.contains(".sw_"), "{css}");
            assert!(!css.contains(":has("), "{css}");
        }
        assert_eq!(
            compile(r"`${trigger()}:active:text-accent`", bounded),
            compile(r#"{ [trigger()]: { "active:": "text-accent" } }"#, bounded)
        );
    }
}

#[test]
fn ref_calls_and_tags_normalize_optional_trailing_separators() {
    for bounded in [false, true] {
        let expected = compile(r#"{ [trigger`hover:active:`]: "text-accent" }"#, bounded);
        for args in [
            r#"{ [trigger`hover:active`]: "text-accent" }"#,
            r#"{ [trigger("hover:active:")]: "text-accent" }"#,
            r#"{ [trigger("hover:active")]: "text-accent" }"#,
            r"`${trigger`hover:active`}:text-accent`",
            r#"`${trigger("hover:active:")}:text-accent`"#,
        ] {
            assert_eq!(compile(args, bounded), expected, "{args}");
        }
        assert!(expected.0.contains(":hover:active"), "{}", expected.0);
        assert!(
            expected.0.contains("@media (hover: hover)"),
            "{}",
            expected.0
        );
    }
}

#[test]
fn direct_ref_interpolation_matches_ordered_nested_wrappers() {
    for bounded in [false, true] {
        for (inline, wrapper) in [
            (
                r"`${trigger`hover:`}:active:text-accent`",
                r#"{ [trigger("hover")]: { "active:": "text-accent" } }"#,
            ),
            (
                r#"`md:${trigger("hover")}:active:text-accent`"#,
                r#"{ "md:": { [trigger`hover:`]: { "active:": "text-accent" } } }"#,
            ),
            (
                r#"`${trigger`hover`}:text-accent p-4 ${trigger("focus")}:opacity-50`"#,
                r#"[{ [trigger("hover")]: "text-accent" }, "p-4", { [trigger`focus:`]: "opacity-50" }]"#,
            ),
            (
                r#"`${trigger("hover")}:${other`focus`}:active:text-accent`"#,
                r#"{ [trigger`hover:`]: { [other("focus:")]: "active:text-accent" } }"#,
            ),
            (
                r#"`[&>p]:${trigger("hover")}:text-accent`"#,
                r#"{ "[&>p]:": { [trigger`hover`]: "text-accent" } }"#,
            ),
            (
                r"active && `${trigger`hover`}:text-accent`",
                r#"{ [trigger("hover")]: active && "text-accent" }"#,
            ),
            (
                r"active ? `${trigger`hover`}:text-accent` : `${trigger`focus`}:text-accent`",
                r#"[{ [trigger("hover")]: active && "text-accent" }, { [trigger("focus")]: !active && "text-accent" }]"#,
            ),
        ] {
            let actual = compile(inline, bounded);
            let expected = compile(wrapper, bounded);
            // A ternary and two separate guards retain their own source guard syntax.
            assert_eq!(actual.0, expected.0, "{inline}");
            if !inline.starts_with("active ?") {
                assert_eq!(actual.1, expected.1, "{inline}");
            }
        }
    }
    let css = compile(r"`${trigger`hover:`}:active:text-accent`", false).0;
    assert!(
        css.contains(":where(.sw_trigger_") && css.contains(":hover *):active"),
        "{css}"
    );
    assert!(css.contains(":active{color:"), "{css}");
    assert!(!css.contains(":where(:hover:active)"), "{css}");
}

#[test]
fn ref_variants_preserve_custom_alternatives_and_arbitrary_selectors() {
    let css = compile(r#"`${trigger("hocus")}:text-accent`"#, false).0;
    assert!(
        css.contains(":where(.sw_trigger_") && css.contains(":hover *)"),
        "{css}"
    );
    assert!(
        css.contains(":where(.sw_trigger_") && css.contains(":focus *)"),
        "{css}"
    );
    let css = compile(
        r#"`${trigger("[&:where(.mod)]")}:active:text-accent`"#,
        true,
    )
    .0;
    assert!(css.contains(":has(:where(.sw_"), "{css}");
    assert!(css.contains(":where(.mod)"), "{css}");
    assert!(css.contains(":active{color:"), "{css}");
    let css = compile(r#"`${trigger("[.mod>&]")}:text-accent`"#, false).0;
    assert!(css.contains(".mod>:where(.sw_"), "{css}");
}

#[test]
fn invalid_ref_variants_report_diagnostics_without_unscoped_output() {
    for args in [
        r#"{ [trigger`&:hover`]: "text-accent" }"#,
        r#"`${trigger("hover::")}:text-accent`"#,
        r#"`${trigger("not-a-variant")}:text-accent`"#,
        r#"`[&:${trigger("hover")}:text-accent]`"#,
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{ style }} from \"stillwind/react\"; const trigger = style.ref(); const props = style({args});"
        )).unwrap();
        let theme =
            crate::css_fixture::source(FileId(2), "@theme { --color-accent:red; }").unwrap();
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
        assert!(!result.diagnostics.is_empty(), "{args}");
        assert!(result.transforms.is_empty(), "{args}");
    }
}

#[test]
fn a_guard_for_multiple_template_utilities_is_evaluated_once() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import {style} from "stillwind/react";
const trigger = style.ref();
const props = style(check() && `${trigger`hover`}:text-accent ${trigger`focus`}:opacity-50`);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme { --color-accent:red; }").unwrap();
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
    let output = result.apply_to_source(&source).unwrap();
    assert_eq!(output.matches("check()").count(), 1, "{output}");
    assert!(result.css.contains(":hover *)"));
    assert!(result.css.contains(":focus *)"));
}

#[test]
fn ref_wrappers_apply_to_keyframes_and_preserve_media_conditions() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import {style} from "stillwind/react";
const trigger = style.ref();
const enter = style.keyframes({ from: "opacity-0", to: "opacity-100" });
const props = style({ [trigger("hover")]: { "active:": enter("duration-120") } });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();
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
    assert!(
        result.css.contains("@media (hover: hover)"),
        "{}",
        result.css
    );
    assert!(result.css.contains(":hover *)"), "{}", result.css);
    assert!(
        result.css.contains(":active{animation-name:"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains_css("animation-duration:120ms;"),
        "{}",
        result.css
    );
}

#[test]
fn an_inline_bounded_ref_keeps_the_ancestry_check() {
    let source = SourceFile::new(FileId(1), None, None, r#"import {style} from "stillwind/react";
const menu = style.ref("menu");
const trigger = style.ref(menu, "trigger");
export const View = () => <><section {...style(menu)}><button {...style(trigger)} /></section><span {...style(`${trigger`hover`}:text-accent`)} /></>;"#).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme { --color-accent:red; }").unwrap();
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
    assert!(
        result
            .lint_findings
            .iter()
            .any(|finding| matches!(finding.kind, LintKind::SelectorOutsideBoundary { .. })),
        "{:?}",
        result.lint_findings
    );
}
