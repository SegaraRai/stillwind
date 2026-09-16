//! Animation and view-transition compilation.
//!
//! Pins keyframe emission for local `@utility` frames, selector-scoped animation uses, keyed view
//! transitions, the independent `--sw-` and `--tw-` prefix configuration, and the rejection of a
//! second view transition in one call.

use stillwind_compiler::CompilerOptions;
use stillwind_css::fold_calc_expressions;
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
        fold_calc_expressions(&result.css)
            .contains("from{--tw-translate-y:var(--spacing-2);translate:var(--tw-translate-x) var(--tw-translate-y);--tw-scale-x:98%;--tw-scale-y:98%;--tw-scale-z:98%;scale:var(--tw-scale-x) var(--tw-scale-y);opacity:0%;}"),
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
    // The token is the class itself; no custom-property indirection remains.
    assert!(result.css.contains("view-transition-class:--sw-v-"));
    assert!(!result.css.contains("--sw-vt-name"));
    assert!(!result.css.contains("--sw-vt-class"));
    let repeated_class = result.classes.last().expect("a class").class_name.as_str();
    assert_eq!(
        result
            .module_css
            .matches(&format!(".{repeated_class}"))
            .count(),
        1,
        "an identical view-transition class should only be emitted once"
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.viewTransition"));
    // A declaration is a plain value, and only a runtime key writes a name inline.
    assert!(
        transformed.contains("const title = {v:\"--sw-v-"),
        "{transformed}"
    );
    assert!(
        transformed.contains("{[stillwindStyleKey]:{\"view-transition-name\":"),
        "{transformed}"
    );
    assert_eq!(
        transformed.matches("stillwindViewTransitionName(").count(),
        1
    );
    assert!(!transformed.contains("stillwindViewTransitionStyle"));
    assert_eq!(transformed.matches("product.id").count(), 1);
}

#[test]
fn compiles_view_transition_conditions_and_image_pair_phases() {
    let component = SourceFile::new(
        FileId(1),
        "src/Gallery.tsx",
        r#"import { style } from "stillwind/react";
const item = style.viewTransition({ old: "opacity-0 motion-reduce:opacity-100", new: "opacity-100", imagePair: "isolation-auto" });
export const photo = style(item);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains("::view-transition-image-pair(.--sw-v-"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains_css("isolation:isolate;") || result.css.contains_css("isolation:auto;"),
        "{}",
        result.css
    );
    let base = result
        .css
        .find("::view-transition-old(.--sw-v-")
        .expect("base old rule");
    let conditional = result
        .css
        .find("@media (prefers-reduced-motion: reduce){::view-transition-old(.--sw-v-")
        .expect("conditional old rule");
    assert!(base < conditional, "{}", result.css);
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
    assert!(result.css.contains("view-transition-class:--app-v-"));
    assert!(result.css.contains("--compat-translate-x"));
    assert!(!result.css.contains("--sw-"));
    assert!(!result.css.contains("--tw-"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("--sw-"));
}

/// Several transitions in one call are one class list: `view-transition-class` is a single
/// property, so the unit names every token rather than adding a second class of its own.
#[test]
fn compiles_guarded_view_transitions_with_complete_class_lists() {
    let component = SourceFile::new(
        FileId(1),
        "src/Guarded.tsx",
        r#"import { style } from "stillwind/react";
declare const compact: boolean;
const main = style.viewTransition({ old: "opacity-0" });
const sub = style.viewTransition({ new: "opacity-100" });
export const item = style("rounded", compact && sub, main.for("hero"));"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    // The unconditional unit declares the classes its own uses contribute, and the guarded
    // unit repeats them: the class property is one declaration, so the later, complete list
    // is the one that applies when the test holds.
    assert_eq!(
        result.css.matches("view-transition-class:").count(),
        2,
        "{}",
        result.css
    );
    let guarded = result
        .css
        .split("view-transition-class:")
        .nth(2)
        .and_then(|rest| rest.split('}').next())
        .expect("a guarded class list");
    assert_eq!(guarded.split_whitespace().count(), 2, "{}", result.css);
    assert!(
        result.css.contains("view-transition-name:--sw-v-"),
        "{}",
        result.css
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("(compact) &&"), "{transformed}");
}

/// A `.for(...)` outside a call builds a value: a literal key folds into a record, and a
/// key only the runtime knows stays a call the runtime resolves.
#[test]
fn lowers_view_transition_key_applications_outside_calls() {
    let component = SourceFile::new(
        FileId(1),
        "src/Keyed.tsx",
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
const card = style.viewTransition({ old: "opacity-0" });
export const literal = card.for("hero");
export const dynamic = card.for(product.id);
export const useLiteral = style("block", literal);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(
        transformed.contains("const literal = {v:\"--sw-v-"),
        "{transformed}"
    );
    assert!(transformed.contains(",k:\"hero\"}"), "{transformed}");
    assert!(
        transformed.contains("stillwindViewTransitionKey(card,product.id)"),
        "{transformed}"
    );
    assert!(
        transformed.contains("stillwindViewTransitionName(literal)"),
        "{transformed}"
    );
}

#[test]
fn unions_view_transition_classes_in_one_call() {
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

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(
        result.css.matches("view-transition-class:").count(),
        1,
        "{}",
        result.css
    );
    let declaration = result
        .css
        .split("view-transition-class:")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("a class list");
    let tokens = declaration
        .split_whitespace()
        .filter(|token| token.starts_with("--sw-v-"))
        .count();
    assert_eq!(tokens, 2, "{}", result.css);
}
