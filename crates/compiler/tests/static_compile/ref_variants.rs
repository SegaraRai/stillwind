//! Ref variants in selectors and templates.
//!
//! Pins that ref presence uses the existing scope or ancestor relation, that call and tag spellings
//! normalize the same way, that direct interpolation matches ordered nested wrappers, that custom
//! alternatives and media conditions survive lowering, and that an unusable ref variant reports a
//! diagnostic instead of emitting an unscoped selector.

use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

fn compile(args: &str, scoped: bool) -> (String, String) {
    let ref_definition = if scoped {
        "const scope = style.scope(\"menu\"); const trigger = style.ref(scope, \"trigger\");"
    } else {
        "const trigger = style.ref(\"trigger\");"
    };
    let use_site = if scoped {
        format!(
            "const View = () => <section {{...style(scope)}}><button {{...style(trigger)}}/><span {{...style({args})}}/></section>;"
        )
    } else {
        format!("const props = style({args});")
    };
    let source = SourceFile::new(FileId(1), "src/RefVariant.tsx", format!(
        "import {{ style }} from \"stillwind/react\"; {ref_definition} const other = style.ref(\"other\"); declare const active: boolean; {use_site}"
    )).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "style.css",
        r#"
@import "stillwind";
@theme { --color-accent: red; }
@variant hocus { &:hover { @slot; } &:focus { @slot; } }
"#,
    )
    .unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
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
fn ref_presence_uses_the_existing_scope_or_ancestor_relation() {
    for scoped in [false, true] {
        let (css, code) = compile(r#"{ [trigger()]: "text-accent" }"#, scoped);
        assert_eq!(
            compile(r"`${trigger()}:text-accent`", scoped),
            (css.clone(), code.clone())
        );
        let utilities = css.split("@layer utilities{").nth(1).unwrap();
        assert!(
            utilities.contains("color:var(--color-accent)"),
            "{utilities}"
        );
        assert!(!utilities.contains(":where("), "{utilities}");
        assert!(!code.contains("trigger()"), "{code}");
        if scoped {
            assert!(css.contains(":has(.s_trigger_"), "{css}");
        } else {
            assert!(css.contains(".s_trigger_"), "{css}");
            assert!(!css.contains(":has("), "{css}");
        }
        assert_eq!(
            compile(r"`${trigger()}:active:text-accent`", scoped),
            compile(r#"{ [trigger()]: { "active:": "text-accent" } }"#, scoped)
        );
    }
}

#[test]
fn ref_calls_and_tags_normalize_optional_trailing_separators() {
    for scoped in [false, true] {
        let expected = compile(r#"{ [trigger`hover:active:`]: "text-accent" }"#, scoped);
        for args in [
            r#"{ [trigger`hover:active`]: "text-accent" }"#,
            r#"{ [trigger("hover:active:")]: "text-accent" }"#,
            r#"{ [trigger("hover:active")]: "text-accent" }"#,
            r"`${trigger`hover:active`}:text-accent`",
            r#"`${trigger("hover:active:")}:text-accent`"#,
        ] {
            assert_eq!(compile(args, scoped), expected, "{args}");
        }
        assert!(
            expected.0.contains(":where(:hover:active)"),
            "{}",
            expected.0
        );
        assert!(
            expected.0.contains("@media (hover: hover)"),
            "{}",
            expected.0
        );
    }
}

#[test]
fn direct_ref_interpolation_matches_ordered_nested_wrappers() {
    for scoped in [false, true] {
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
            let actual = compile(inline, scoped);
            let expected = compile(wrapper, scoped);
            // A ternary and two separate guards retain their own source guard syntax.
            assert_eq!(actual.0, expected.0, "{inline}");
            if !inline.starts_with("active ?") {
                assert_eq!(actual.1, expected.1, "{inline}");
            }
        }
    }
    let css = compile(r"`${trigger`hover:`}:active:text-accent`", false).0;
    assert!(css.contains(":where(:hover) ._s_"), "{css}");
    assert!(css.contains(":active{color:"), "{css}");
    assert!(!css.contains(":where(:hover:active)"), "{css}");
}

#[test]
fn ref_variants_preserve_custom_alternatives_and_arbitrary_selectors() {
    let css = compile(r#"`${trigger("hocus")}:text-accent`"#, false).0;
    assert!(css.contains(":where(:hover) ._s_"), "{css}");
    assert!(css.contains(":where(:focus) ._s_"), "{css}");
    let css = compile(
        r#"`${trigger("[&:where(.mod)]")}:active:text-accent`"#,
        true,
    )
    .0;
    assert!(css.contains(":has(.s_trigger_"), "{css}");
    assert!(css.contains(":where(:where(.mod))"), "{css}");
    assert!(css.contains(":active{color:"), "{css}");
    let css = compile(r#"`${trigger("[.mod>&]")}:text-accent`"#, false).0;
    assert!(css.contains(":where(.mod>.s_trigger_"), "{css}");
}

#[test]
fn invalid_ref_variants_report_diagnostics_without_unscoped_output() {
    for args in [
        r#"{ [trigger`&:hover`]: "text-accent" }"#,
        r#"`${trigger("hover::")}:text-accent`"#,
        r#"`${trigger("not-a-variant")}:text-accent`"#,
        r#"`[&:${trigger("hover")}:text-accent]`"#,
    ] {
        let source = SourceFile::new(FileId(1), "src/Invalid.tsx", format!(
            "import {{ style }} from \"stillwind/react\"; const trigger = style.ref(); const props = style({args});"
        )).unwrap();
        let theme =
            crate::css_fixture::source(FileId(2), "style.css", "@theme { --color-accent:red; }")
                .unwrap();
        let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
        assert!(!result.diagnostics.is_empty(), "{args}");
        assert!(result.transforms.is_empty(), "{args}");
    }
}

#[test]
fn a_guard_for_multiple_template_utilities_is_evaluated_once() {
    let source = SourceFile::new(
        FileId(1),
        "src/Guard.tsx",
        r#"import {style} from "stillwind/react";
const trigger = style.ref();
const props = style(check() && `${trigger`hover`}:text-accent ${trigger`focus`}:opacity-50`);"#,
    )
    .unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "style.css", "@theme { --color-accent:red; }")
            .unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.apply_to_source(&source).unwrap();
    assert_eq!(output.matches("check()").count(), 1, "{output}");
    assert!(result.css.contains(":where(:hover)"));
    assert!(result.css.contains(":where(:focus)"));
}

#[test]
fn ref_wrappers_apply_to_animations_and_preserve_media_conditions() {
    let source = SourceFile::new(
        FileId(1),
        "src/Motion.tsx",
        r#"import {style} from "stillwind/react";
const trigger = style.ref();
const enter = style.animation({ from: "opacity-0", to: "opacity-100" });
const props = style({ [trigger("hover")]: { "active:": enter("duration-120") } });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "style.css", "@theme {}").unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.css.contains("@media (hover: hover)"),
        "{}",
        result.css
    );
    assert!(result.css.contains(":where(:hover) ._s_"), "{}", result.css);
    assert!(
        result.css.contains(":active{animation-name:"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("animation-duration:120ms;"),
        "{}",
        result.css
    );
}

#[test]
fn an_inline_scoped_ref_keeps_the_scope_ancestry_check() {
    let source = SourceFile::new(FileId(1), "src/Scope.tsx", r#"import {style} from "stillwind/react";
const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");
export const View = () => <><section {...style(menu)}><button {...style(trigger)} /></section><span {...style(`${trigger`hover`}:text-accent`)} /></>;"#).unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "style.css", "@theme { --color-accent:red; }")
            .unwrap();
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
    assert!(
        result
            .lint_findings
            .iter()
            .any(|finding| finding.rule == "scoped-selector-in-scope"),
        "{:?}",
        result.lint_findings
    );
}
