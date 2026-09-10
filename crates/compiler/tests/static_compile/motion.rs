use stillwind_compiler::CompilerOptions;
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn compiles_local_utility_animations_and_selector_uses() {
    let component = SourceFile::new(
        FileId(1),
        "src/Dialog.tsx",
        r#"import { style } from "stillwind/react";
const enter = style.animation({ from: "opacity-0 translate-y-2 scale-98", to: "opacity-100 translate-y-0 scale-100" });
const exit = style.animation({ from: "opacity-100", to: "opacity-0 scale-98" });
const panel = style("rounded-md", enter("duration-180 ease-standard both"), { "[&[data-state=closed]]:": exit("duration-120 ease-standard both") });"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --radius-md: .375rem; --spacing-2: .5rem; --ease-standard: ease-out; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@keyframes --sw-a-"));
    assert!(
        result
            .css
            .contains("from{opacity:0%;--tw-translate-y:var(--spacing-2);translate:var(--tw-translate-x) var(--tw-translate-y);--tw-scale-x:98%;--tw-scale-y:98%;--tw-scale-z:98%;scale:var(--tw-scale-x) var(--tw-scale-y);}"),
        "{}",
        result.css
    );
    assert!(result.css.contains("animation-name:--sw-a-"));
    assert!(result.css.contains("animation-duration:"));
    assert!(
        result
            .css
            .contains("[data-state=closed]{animation-name:--sw-a-")
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.animation"));
    assert!(transformed.contains("const enter = undefined;"));
}

#[test]
fn compiles_view_transition_assets_and_keyed_instances() {
    let component = SourceFile::new(
        FileId(1),
        "src/ProductCard.tsx",
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
const title = style.viewTransition({ group: "duration-180 ease-standard", old: "opacity-0 -translate-y-1", new: "opacity-100" });
const card = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
const heading = style("text-accent", title);
const item = style("rounded-md", card.for(product.id));
const image = style(title);
const detail = style(title);"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --radius-md: .375rem; --spacing-1: .25rem; --ease-standard: ease-out; }",
    ).unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("::view-transition-group(.--sw-v-"));
    assert!(result.css.contains("::view-transition-old(.--sw-v-"));
    assert!(result.css.contains("::view-transition-new(.--sw-v-"));
    assert!(
        result
            .css
            .contains("view-transition-name:var(--sw-vt-name);")
    );
    assert!(
        result
            .css
            .contains("view-transition-class:var(--sw-vt-class);")
    );
    let repeated_class = result.classes[result.classes.len() - 1].class_name.as_str();
    assert_eq!(
        result
            .module_css
            .matches(&format!(".{repeated_class}"))
            .count(),
        1,
        "an identical view-transition utility class should only be emitted once"
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.viewTransition"));
    assert_eq!(
        transformed.matches("stillwindViewTransitionStyle(").count(),
        4
    );
    assert_eq!(transformed.matches("product.id").count(), 1);
    assert!(transformed.contains("const title = undefined;"));
}

#[test]
fn configures_stillwind_and_tailwind_compatibility_prefixes_independently() {
    let component = SourceFile::new(
        FileId(1),
        "src/Prefix.tsx",
        r#"import { style } from "stillwind/react";
const card = style.viewTransition({ old: "opacity-0" });
export const item = style("translate-x-1", card);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-1: .25rem; }",
    )
    .unwrap();
    let options = CompilerOptions {
        prefix: "--app-".to_owned(),
        tailwind_prefix: "--compat-".to_owned(),
        ..CompilerOptions::default()
    };

    let result =
        Compiler::compile_source_with_options(&OxcSourceAnalyzer, &component, &theme, &options);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("::view-transition-old(.--app-v-"));
    assert!(result.css.contains("var(--app-vt-name)"));
    assert!(result.css.contains("--compat-translate-x"));
    assert!(!result.css.contains("--sw-"));
    assert!(!result.css.contains("--tw-"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("\"--app-vt-name\""));
    assert!(!transformed.contains("--sw-"));
}

#[test]
fn rejects_multiple_view_transitions_on_one_element() {
    let component = SourceFile::new(
        FileId(1),
        "src/MultipleTransitions.tsx",
        r#"import { style } from "stillwind/react";
const card = style.viewTransition({ old: "opacity-0" });
const image = style.viewTransition({ new: "opacity-100" });
export const item = style(card, image);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SHVIEW004")
    );
}
