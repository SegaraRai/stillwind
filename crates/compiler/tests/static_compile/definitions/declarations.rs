//! Checks animation declarations, labels, and compatibility prefixes.

use stillwind_compiler::CompileResult;

use super::*;

#[test]
fn compiles_local_utility_animations_and_selector_uses() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const enter = style.keyframes({ from: "opacity-0 translate-y-2 scale-98", to: "opacity-100 translate-y-0 scale-100" });
const exit = style.keyframes({ from: "opacity-100", to: "opacity-0 scale-98" });
const panel = style("rounded-md", enter("duration-180 ease-standard both"), { "[&[data-state=closed]]:": exit("duration-120 ease-standard both") });"#,
    ).unwrap();
    let theme = css_source(
        FileId(2),
        "@theme { --radius-md: .375rem; --spacing-2: .5rem; --ease-standard: ease-out; }",
    )
    .unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@keyframes sw-kf-"));
    // Frames write the typed `--sw-motion-*` registrations, which interpolate, instead of
    // the `--tw-*` variables of element utilities.
    let css = fold_calc_expressions(&result.css, None);
    for expected in [
        "from{opacity:0%;--sw-motion-translate-y:var(--spacing-2);translate:var(--sw-motion-translate-x) var(--sw-motion-translate-y)",
        "--sw-motion-scale-x:calc(98 / 100);--sw-motion-scale-y:calc(98 / 100);scale:var(--sw-motion-scale-x) var(--sw-motion-scale-y) var(--sw-motion-scale-z);",
        "@property --sw-motion-translate-y {
  syntax: \"<length-percentage>\";",
    ] {
        assert!(
            css.contains(expected),
            "{expected}
{css}"
        );
    }
    assert!(!css.contains("--tw-translate"), "{css}");
    assert!(result.css.contains("animation-name:sw-kf-"));
    assert!(result.css.contains("animation-duration:"));
    assert!(
        result
            .css
            .contains("[data-state=closed]{animation-name:sw-kf-")
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.keyframes"));
    assert!(transformed.contains("const enter = undefined;"));
}

/// A declaration's label is spelled into every name the declaration generates, in the family's
/// own separator, and the rendering positions write it the way their grammar requires.
#[test]
fn spells_declaration_labels_into_the_generated_names() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const enter = style.keyframes({ label: "fade", from: "opacity-0", to: "opacity-100" });
export const panel = style.viewTransition({ label: "panel", old: "opacity-0" });
export const item = style(panel, enter("duration-180"));"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains("@keyframes sw-kf-fade-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("animation-name:sw-kf-fade-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("::view-transition-old(.sw-vt-panel-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("view-transition-class:sw-vt-panel-"),
        "{}",
        result.css
    );
    // The declaration value carries the same token the rules select.
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("{v:\"sw-vt-panel-"), "{transformed}");
    assert!(!transformed.contains("style.keyframes"), "{transformed}");
}

#[test]
fn configures_stillwind_and_tailwind_compatibility_prefixes_independently() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const card = style.viewTransition({ old: "opacity-0" });
export const item = style("translate-x-1", card);"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme { --spacing-1: .25rem; }").unwrap();
    let options = CompilerOptions {
        prefix: ("--app-".to_owned()).into(),
        tailwind_prefix: ("--compat-".to_owned()).into(),
        ..CompilerOptions::default()
    };

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &options,
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    // The class token is not a custom property, so the custom-property prefix does not
    // namespace it; the compatibility prefix still reaches the Tailwind properties.
    assert!(result.css.contains("::view-transition-old(.sw-vt-"));
    assert!(result.css.contains("view-transition-class:sw-vt-"));
    assert!(result.css.contains("--compat-translate-x"));
    assert!(!result.css.contains("--sw-"));
    assert!(!result.css.contains("--tw-"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("--sw-"));
}

/// Compiles `source` with `css` and `options` for the theme prefix tests.
fn compile_themed(source: &str, css: &str, options: &CompilerOptions) -> CompileResult {
    let component = SourceFile::new(FileId(1), None, None, source).unwrap();
    let theme = css_source(FileId(2), css).unwrap();
    Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        options,
    )
}

#[test]
fn theme_prefix_renames_definitions_and_every_reference() {
    let result = compile_themed(
        r#"import { style } from "stillwind/react";
export const item = style("bg-brand bg-[var(--color-arbitrary)] bg-(--tw-color-hint) shadow-md translate-x-1 animate-pulse-once");"#,
        r#"@theme {
  --color-brand: red;
  --color-arbitrary: green;
  --color-hint: blue;
  --color-rule: gold;
  --color-prefixed: pink;
  --color-unused: black;
  --animate-pulse-once: pulse-once 1s;
  @keyframes pulse-once { to { opacity: 0; } }
}
.rule { color: var(--color-rule); border-color: var(--tw-color-prefixed); content: "--color-rule"; }"#,
        &CompilerOptions {
            theme_prefix: "--tw-".into(),
            ..CompilerOptions::default()
        },
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    for expected in [
        "--tw-color-brand:red",
        "--tw-color-arbitrary:green",
        "--tw-color-hint:blue",
        "--tw-color-rule:gold",
        "--tw-color-prefixed:pink",
        "--tw-animate-pulse-once:pulse-once 1s",
        "var(--tw-color-brand)",
        "var(--tw-color-arbitrary)",
        "var(--tw-color-rule)",
        "var(--tw-color-prefixed)",
        "@keyframes pulse-once",
        "\"--color-rule\"",
        "--tw-translate-x",
    ] {
        assert!(result.css.contains(expected), "{expected}: {}", result.css);
    }
    assert!(!result.css.contains("--tw-tw-"), "{}", result.css);
    assert!(!result.css.contains("var(--color-"), "{}", result.css);
    assert!(!result.css.contains("color-unused"), "{}", result.css);
}

#[test]
fn theme_prefix_is_independent_of_the_compatibility_prefix() {
    let result = compile_themed(
        r#"import { style } from "stillwind/react";
export const item = style("bg-brand translate-x-1");"#,
        "@theme { --color-brand: red; --spacing-1: .25rem; }",
        &CompilerOptions {
            tailwind_prefix: "--compat-".into(),
            theme_prefix: "--tw-".into(),
            ..CompilerOptions::default()
        },
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("--tw-color-brand"), "{}", result.css);
    assert!(
        result.css.contains("--compat-translate-x"),
        "{}",
        result.css
    );
    assert!(!result.css.contains("--tw-translate-x"), "{}", result.css);
}

#[test]
fn theme_prefix_rejects_a_variable_with_the_name_of_an_internal_property() {
    let result = compile_themed(
        r#"import { style } from "stillwind/react";
export const item = style("blur");"#,
        "@theme { --blur: 3px; }",
        &CompilerOptions {
            theme_prefix: "--tw-".into(),
            ..CompilerOptions::default()
        },
    );

    assert!(
        result.diagnostics.iter().any(|diagnostic| matches!(
            &diagnostic.diagnostic,
            CompileDiagnostic::ThemePrefixCollision { token, property }
                if token == "--blur" && property == "--tw-blur"
        )),
        "{:#?}",
        result.diagnostics
    );
}
