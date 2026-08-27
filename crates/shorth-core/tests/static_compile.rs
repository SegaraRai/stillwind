use shorth_core::Compiler;
use shorth_source::{FileId, SourceFile};

#[test]
fn compiles_a_real_react_style_call_against_a_css_theme() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";

export function SaveButton() {
  return <button {...style("inline-flex items-center rounded-md bg-accent px-4 text-center")}>Save</button>;
}"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; --radius-md: .375rem; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 1);
    assert_eq!(
        result.css,
        "@layer theme, base, components, utilities;\n@layer theme{:root{--color-accent:oklch(.6 .2 250);--spacing-4:1rem;--radius-md:.375rem;}}\n@layer utilities{._s_bc6dda5a{display:inline-flex;align-items:center;border-radius:var(--radius-md);background-color:var(--color-accent);padding-inline:var(--spacing-4);text-align:center;}}\n",
    );
}

#[test]
fn reports_an_unknown_token_at_the_utility_not_the_whole_call() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accnet");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);
    let diagnostic = result
        .diagnostics
        .first()
        .expect("an unknown utility diagnostic");

    assert_eq!(diagnostic.code, "SH001");
    assert_eq!(
        component.slice(diagnostic.labels[0].span).unwrap(),
        "bg-accnet"
    );
    assert!(result.css.is_empty());
}

#[test]
fn emits_css_for_a_runtime_conditional_branch_without_evaluating_it() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("opacity:.6;"));
}

#[test]
fn plans_react_props_for_classes_a_conditional_and_an_aria_attribute() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60", { "aria-busy": loading });"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.transforms.len(), 1);
    let transform = &result.transforms[0];
    assert_eq!(transform.call_span, result.classes[0].call_span);
    assert_eq!(
        transform.replacement,
        format!(
            "{{className:stillwindClsx(\"{}\",loading&&\"{}\"),\"aria-busy\":loading}}",
            result.classes[0].class_name, result.classes[1].class_name,
        ),
    );
}

#[test]
fn emits_tailwind_shaped_layers_theme_variables_and_variants() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90 md:px-4 aria-[busy=true]:opacity-60");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        r"@theme {
  --color-accent: oklch(.6 .2 250);
  --spacing-4: 1rem;
  --breakpoint-md: 48rem;
}",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .starts_with("@layer theme, base, components, utilities;\n")
    );
    assert!(result.css.contains("@layer theme{:root{--color-accent:oklch(.6 .2 250);--spacing-4:1rem;--breakpoint-md:48rem;}}"));
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(
        result
            .css
            .contains("background-color:color-mix(in oklab,var(--color-accent) 90%,transparent);")
    );
    assert!(result.css.contains(":hover{"));
    assert!(result.css.contains("[aria-busy=\"true\"]{"));
    assert!(result.css.contains("@media (width >= 48rem){"));
}

#[test]
fn applies_a_transform_plan_without_touching_surrounding_source() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent");
export { props };"#,
    );
    let theme = SourceFile::new(FileId(2), "shorth.css", "@theme { --color-accent: red; }");

    let result = Compiler::compile_react(&component, &theme);
    let transformed = result.apply_to_source(&component).unwrap();

    assert!(
        transformed.starts_with(
            "import { style } from \"stillwind/react\";\nconst props = {className:\"_s_"
        )
    );
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
  { "aria-busy": loading, "&:hover": "bg-accent/90" },
);"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; --spacing-4: 1rem; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("padding-inline:var(--spacing-4);"));
    assert!(result.css.contains(":hover{background-color:color-mix("));
    assert!(result.css.contains("opacity:.6;"));
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
const props = style("-mt-4 duration-fast ease-standard");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --spacing-4: 1rem; --duration-fast: 120ms; --ease-standard: ease-out; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("margin-top:calc(var(--spacing-4) * -1);")
    );
    assert!(
        result
            .css
            .contains("transition-duration:var(--duration-fast);")
    );
    assert!(
        result
            .css
            .contains("transition-timing-function:var(--ease-standard);")
    );
}

#[test]
fn projects_one_handle_into_anchor_position_scope_and_container_css() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.handle();
const root = style(popover.scope, popover.container("inline-size"));
const trigger = style(popover.anchor, "relative");
const panel = style(popover.position, "absolute");"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("anchor-name:--sw-h-"));
    assert!(result.css.contains("position-anchor:--sw-h-"));
    assert!(result.css.contains("anchor-scope:--sw-h-"));
    assert!(result.css.contains("container-name:--sw-h-"));
    assert!(result.css.contains("container-type:inline-size;"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.handle()"));
    assert!(transformed.contains("const popover = undefined;"));
}

#[test]
fn reports_a_position_handle_without_an_anchor_provider() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.handle();
const panel = style(popover.position);"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.code == "SHHANDLE003")
    );
    assert!(result.css.is_empty());
}

#[test]
fn emits_named_container_rules_from_handle_queries() {
    let component = SourceFile::new(
        FileId(1),
        "src/Card.tsx",
        r#"import { style } from "stillwind/react";
const card = style.handle();
const root = style(card.container("inline-size"));
const content = style(card.at("width >= 40rem", "grid-cols-2"));"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@container --sw-h-"));
    assert!(result.css.contains("(width >= 40rem){"));
    assert!(
        result
            .css
            .contains("grid-template-columns:repeat(2,minmax(0,1fr));")
    );
}
