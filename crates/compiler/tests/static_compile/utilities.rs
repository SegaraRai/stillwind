use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn emits_tailwind_shaped_layers_theme_variables_and_variants() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90 md:px-4 aria-[busy=true]:opacity-60");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        r"@theme {
  --color-accent: oklch(.6 .2 250);
  --spacing-4: 1rem;
  --breakpoint-md: 48rem;
}",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .starts_with("@layer theme, base, components, utilities;")
    );
    assert!(!result.css.contains("@layer properties;"));
    assert!(result.css.contains("--color-accent:oklch(.6 .2 250);"));
    assert!(result.css.contains("--spacing-4:1rem;"));
    assert!(!result.css.contains("--breakpoint-md:48rem;"));
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("background-color:color-mix(in oklab"));
    assert!(result.css.contains(":hover{"));
    assert!(result.css.contains("[aria-busy=\"true\"]{"));
    assert!(result.css.contains("@media (width >= 48rem){"));
}

#[test]
fn retains_authored_custom_properties_and_theme_selectors() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        r#"@theme inline { --color-accent: var(--app-accent); }
:root { --app-accent: oklch(.6 .2 250); }
[data-theme="dark"] { --app-accent: oklch(.3 .1 250); }"#,
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(!result.css.contains("--color-accent:var(--app-accent);"));
    assert!(
        result
            .css
            .contains(":root { --app-accent: oklch(.6 .2 250); }")
    );
    assert!(
        result
            .css
            .contains("[data-theme=\"dark\"] { --app-accent: oklch(.3 .1 250); }")
    );
}

#[test]
fn resolves_v4_default_spacing_and_arbitrary_custom_properties() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("px-4 -mt-2 bg-red-500 [--gutter:1rem] w-[calc(var(--gutter)*2)]");"#,
    )
    .unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "src/styles/stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("padding-inline:calc(var(--spacing) * 4);")
    );
    assert!(result.css.contains("margin-top:calc(var(--spacing) * -2);"));
    assert!(
        result
            .css
            .contains("background-color:var(--color-red-500);")
    );
    assert!(result.css.contains("--gutter:1rem;"));
    assert!(result.css.contains("width:calc(var(--gutter) * 2);"));
}

#[test]
fn resolves_v4_default_animations_and_emits_their_keyframes() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("animate-spin");"#,
    )
    .unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "src/styles/stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("animation:var(--animate-spin);"));
    assert!(result.css.contains("@keyframes spin"));
}

#[test]
fn resolves_css_defined_custom_utilities_and_variants() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("content-auto data-checked:content-auto tab-8");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        r#"@utility content-auto { content-visibility: auto; }
@utility tab-* { tab-size: --value(integer); }
@custom-variant data-checked (&[data-ui~="checked"]);"#,
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("content-visibility: auto;"));
    assert!(result.css.contains("[data-ui~=\"checked\"]{"));
    assert!(result.css.contains("tab-size: 8;"));
}

#[test]
fn applies_a_transform_plan_without_touching_surrounding_source() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent");
export { props };"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);
    let transformed = result.apply_to_source(&component).unwrap();

    assert!(transformed.starts_with("\nconst props = {className:\"_s_"));
    assert!(transformed.ends_with("};\nexport { props };"));
}

#[test]
fn compiles_nested_arrays_attributes_and_element_local_selectors() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style(
  ["bg-accent", ["px-4"], loading && "opacity-60"],
  { "aria-busy": loading, "hover:": "bg-accent/90" },
);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; --spacing-4: 1rem; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("padding-inline:var(--spacing-4);"));
    assert!(result.css.contains(":hover{background-color:color-mix("));
    assert!(result.css.contains("opacity:60%;"), "{}", result.css);
    assert!(
        result.transforms[0]
            .replacement
            .contains("\"aria-busy\":loading")
    );
}

#[test]
fn resolves_negative_and_transition_utilities_from_declared_theme_tokens() {
    let component = SourceFile::new(
        FileId(1),
        "src/Card.tsx",
        r#"import { style } from "stillwind/react";
const props = style("-mt-4 duration-120 ease-standard");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --spacing-4: 1rem; --ease-standard: ease-out; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("margin-top:calc(var(--spacing-4) * -1);")
    );
    assert!(
        result.css.contains("--tw-duration:120ms;"),
        "{}",
        result.css
    );
    assert!(result.css.contains("--tw-ease:var(--ease-standard);"));
}
