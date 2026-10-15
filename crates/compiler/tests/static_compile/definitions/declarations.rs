//! Checks animation declarations, labels, and compatibility prefixes.

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
    assert!(
        fold_calc_expressions(&result.css, None)
            .contains("from{opacity:0%;--tw-translate-y:var(--spacing-2);translate:var(--tw-translate-x) var(--tw-translate-y);--tw-scale-x:98%;--tw-scale-y:98%;--tw-scale-z:98%;scale:var(--tw-scale-x) var(--tw-scale-y);}"),
        "{}",
        result.css
    );
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
const panel = style.viewTransition({ label: "panel", old: "opacity-0" });
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
