use shorth_core::Compiler;
use shorth_source::{FileId, SourceFile};

#[test]
fn compiles_a_real_react_style_call_against_a_css_theme() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "shorth/react";

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
        "._s_bc6dda5a{display:inline-flex;align-items:center;border-radius:var(--radius-md);background-color:var(--color-accent);padding-inline:var(--spacing-4);text-align:center;}\n",
    );
}

#[test]
fn reports_an_unknown_token_at_the_utility_not_the_whole_call() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "shorth/react";
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
        r#"import { style } from "shorth/react";
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
        r#"import { style } from "shorth/react";
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
            "{{className:shorthClsx(\"{}\",loading&&\"{}\"),\"aria-busy\":loading}}",
            result.classes[0].class_name, result.classes[1].class_name,
        ),
    );
}
