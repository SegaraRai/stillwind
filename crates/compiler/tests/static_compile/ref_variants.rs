//! Ref variants in selectors and templates.
//!
//! Pins that ref presence uses the scope containment or ancestor relation, that call
//! and tag spellings normalize the same way, that direct interpolation matches ordered
//! nested wrappers, that custom alternatives and media conditions survive lowering, and
//! that an unusable ref variant reports a diagnostic instead of emitting an unscoped
//! selector.

use std::sync::OnceLock;

use stillwind_compiler::CompilerOptions;
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::assertions::CssAssertions as _;
use stillwind_test_support::css;

use super::*;

fn compile(args: &str, scoped: bool) -> (String, String) {
    static SNAPSHOT: OnceLock<DesignSystemSnapshot> = OnceLock::new();

    let args = if scoped {
        args.to_owned()
    } else {
        args.replace(".in(menu)", "")
    };
    let ref_definition = if scoped {
        "const menu = style.ref(\"menu\"); const trigger = style.ref(\"trigger\");"
    } else {
        "const trigger = style.ref(\"trigger\");"
    };
    let use_site = if scoped {
        format!(
            "const View = () => <section {{...style(menu)}}><button {{...style(trigger)}}/><span {{...style({args})}}/></section>;"
        )
    } else {
        format!("const props = style({args});")
    };
    let source = SourceFile::new(FileId(1), None, None, format!(
        "import {{ style }} from \"stillwind/react\"; {ref_definition} const other = style.ref(\"other\"); declare const active: boolean; {use_site}"
    )).unwrap();
    let snapshot = SNAPSHOT.get_or_init(|| {
        let theme = css::source(
            FileId(2),
            r#"
@import "stillwind";
@theme { --color-accent: red; }
@variant hocus { &:hover { @slot; } &:focus { @slot; } }
"#,
        )
        .unwrap();
        DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("stillwind.css"),
            CompilerOptions::default().design_system(),
        )
        .unwrap()
    });
    let result = compile_with_snapshot(&source, snapshot, "test.tsx", ScriptLanguage::Tsx);
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
    let mut css = result.css.clone();
    let mut code = result.apply_to_source(&source).unwrap();
    for (index, class) in result.classes.iter().enumerate() {
        let normalized = format!("test_class_{index}");
        css = css.replace(&format!(".{}", class.class_name), &format!(".{normalized}"));
        code = code.replace(&class.class_name, &normalized);
    }
    (css, code)
}

#[test]
fn ref_presence_uses_scope_containment_or_ancestor_relation() {
    for scoped in [false, true] {
        let (css, code) = compile(r#"{ [trigger().in(menu)]: "text-accent" }"#, scoped);
        assert_eq!(
            compile(r"`${trigger().in(menu)}:text-accent`", scoped),
            (css.clone(), code.clone())
        );
        let utilities = css.split("@layer utilities{").nth(1).unwrap();
        assert!(
            utilities.contains("color:var(--color-accent)"),
            "{utilities}"
        );
        assert!(utilities.contains(":where("), "{utilities}");
        assert!(!code.contains("trigger().in(menu)"), "{code}");
        if scoped {
            assert!(css.contains(":has(:is(:where(.sw_"), "{css}");
        } else {
            assert!(css.contains(".sw_"), "{css}");
            assert!(!css.contains(":has("), "{css}");
        }
        assert_eq!(
            compile(r"`${trigger().in(menu)}:active:text-accent`", scoped),
            compile(
                r#"{ [trigger().in(menu)]: { "active:": "text-accent" } }"#,
                scoped
            )
        );
    }
}

#[test]
fn ref_calls_and_tags_normalize_optional_trailing_separators() {
    for scoped in [false, true] {
        let expected = compile(
            r#"{ [trigger`hover:active:`.in(menu)]: "text-accent" }"#,
            scoped,
        );
        for args in [
            r#"{ [trigger`hover:active`.in(menu)]: "text-accent" }"#,
            r#"{ [trigger("hover:active:").in(menu)]: "text-accent" }"#,
            r#"{ [trigger("hover:active").in(menu)]: "text-accent" }"#,
            r"`${trigger`hover:active`.in(menu)}:text-accent`",
            r#"`${trigger("hover:active:").in(menu)}:text-accent`"#,
        ] {
            assert_eq!(compile(args, scoped), expected, "{args}");
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
    for scoped in [false, true] {
        for (inline, wrapper) in [
            (
                r"`${trigger`hover:`.in(menu)}:active:text-accent`",
                r#"{ [trigger("hover").in(menu)]: { "active:": "text-accent" } }"#,
            ),
            (
                r#"`md:${trigger("hover").in(menu)}:active:text-accent`"#,
                r#"{ "md:": { [trigger`hover:`.in(menu)]: { "active:": "text-accent" } } }"#,
            ),
            (
                r#"`${trigger`hover`.in(menu)}:text-accent p-4 ${trigger("focus").in(menu)}:opacity-50`"#,
                r#"[{ [trigger("hover").in(menu)]: "text-accent" }, "p-4", { [trigger`focus:`.in(menu)]: "opacity-50" }]"#,
            ),
            (
                r#"`${trigger("hover").in(menu)}:${other`focus`}:active:text-accent`"#,
                r#"{ [trigger`hover:`.in(menu)]: { [other("focus:")]: "active:text-accent" } }"#,
            ),
            (
                r#"`[&>p]:${trigger("hover").in(menu)}:text-accent`"#,
                r#"{ "[&>p]:": { [trigger`hover`.in(menu)]: "text-accent" } }"#,
            ),
            (
                r"active && `${trigger`hover`.in(menu)}:text-accent`",
                r#"{ [trigger("hover").in(menu)]: active && "text-accent" }"#,
            ),
            (
                r"active ? `${trigger`hover`.in(menu)}:text-accent` : `${trigger`focus`.in(menu)}:text-accent`",
                r#"[{ [trigger("hover").in(menu)]: active && "text-accent" }, { [trigger("focus").in(menu)]: !active && "text-accent" }]"#,
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
    let css = compile(r"`${trigger`hover:`.in(menu)}:active:text-accent`", false).0;
    assert!(
        css.contains(":where(.sw_trigger_") && css.contains(":hover) *)"),
        "{css}"
    );
    assert!(css.contains(":active:is("), "{css}");
    assert!(!css.contains(":where(:hover:active)"), "{css}");
}

#[test]
fn ref_variants_preserve_custom_alternatives_and_arbitrary_selectors() {
    let css = compile(r#"`${trigger("hocus").in(menu)}:text-accent`"#, false).0;
    assert!(
        css.contains(":where(.sw_trigger_") && css.contains(":hover) *)"),
        "{css}"
    );
    assert!(
        css.contains(":where(.sw_trigger_") && css.contains(":focus) *)"),
        "{css}"
    );
    let css = compile(
        r#"`${trigger("[&:where(.mod)]").in(menu)}:active:text-accent`"#,
        true,
    )
    .0;
    assert!(css.contains(":has(:is(:where(.sw_"), "{css}");
    assert!(css.contains(":where(.mod)"), "{css}");
    assert!(css.contains(":active:is("), "{css}");
    let css = compile(r#"`${trigger("[.mod>&]").in(menu)}:text-accent`"#, false).0;
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
        let theme = css::source(FileId(2), "@theme { --color-accent:red; }").unwrap();
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
    let theme = css::source(FileId(2), "@theme { --color-accent:red; }").unwrap();
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
    assert!(result.css.contains(":hover) *)"));
    assert!(result.css.contains(":focus) *)"));
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
    let theme = css::source(FileId(2), "@theme {}").unwrap();
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
    assert!(result.css.contains(":hover) *)"), "{}", result.css);
    assert!(
        result.css.contains(":active:is(") && result.css.contains("animation-name:"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains_css("animation-duration:120ms;"),
        "{}",
        result.css
    );
}
