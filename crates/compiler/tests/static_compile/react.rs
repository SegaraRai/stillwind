use super::*;

#[test]
fn compiles_a_real_react_style_call_against_a_css_theme() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";

export function SaveButton() {
  return <button {...style("inline-flex items-center rounded-md bg-accent px-4 text-center")}>Save</button>;
}"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; --radius-md: .375rem; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 1);
    assert!(
        result
            .css
            .starts_with("@layer theme, base, components, utilities;")
    );
    assert!(!result.css.contains("@layer properties;"));
    assert!(result.css.contains("--color-accent:oklch(.6 .2 250);"));
    assert!(result.css.contains("@layer base"));
    assert!(result.css.contains("._s_bc6dda5a{display:inline-flex;padding-inline:var(--spacing-4);align-items:center;text-align:center;background-color:var(--color-accent);border-radius:var(--radius-md);}"), "{}", result.css);
}

#[test]
fn reports_an_unknown_token_at_the_utility_not_the_whole_call() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accnet");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);
    let finding = result
        .lint_findings
        .first()
        .expect("an unknown utility finding");

    assert_eq!(finding.rule, "unknown-utility");
    assert_eq!(
        component.slice(finding.labels[0].span).unwrap(),
        "bg-accnet"
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("bg-accnet"));
    assert!(!result.css.is_empty());
}

#[test]
fn preserves_unparseable_tailwind_candidates_as_lint_findings() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-[color:red");"#,
    )
    .unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "stillwind.css", "@import \"stillwind\";").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);
    assert_eq!(result.lint_findings[0].rule, "invalid-utility");
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("bg-[color:red"));
}

#[test]
fn honors_swlint_disable_directives_by_source_location() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
// swlint-disable-next-line unknown-utility
const first = style("bg-accnet");
const second = style("text-accnet"); // swlint-disable-line unknown-utility
/* swlint-disable unknown-utility */
const third = style("p-accnet");
/* swlint-enable unknown-utility */
const fourth = style("m-accnet");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);
    assert_eq!(result.lint_findings.len(), 1);
    assert_eq!(result.lint_findings[0].rule, "unknown-utility");
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("bg-accnet"));
    assert!(transformed.contains("text-accnet"));
    assert!(transformed.contains("p-accnet"));
    assert!(transformed.contains("m-accnet"));
}

#[test]
fn emits_css_for_a_runtime_conditional_branch_without_evaluating_it() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("opacity:60%;"), "{}", result.css);
}

#[test]
fn compiles_both_sides_of_a_static_ternary_branch() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const active: boolean;
const props = style(active ? "bg-accent" : "bg-muted");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --color-muted: gray; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("background-color:var(--color-muted);"));
    assert!(result.transforms[0].replacement.contains("active&&"));
    assert!(result.transforms[0].replacement.contains("!active&&"));
}

#[test]
fn compound_branch_guards_preserve_precedence_for_both_frontends() {
    for frontend in ["react", "solid"] {
        let component = SourceFile::new(
            FileId(1),
            "src/Branch.tsx",
            format!(
                r#"import {{ style }} from "stillwind/{frontend}";
const props = style(n() > 0 ? "bg-accent" : "bg-muted");"#
            ),
        )
        .unwrap();
        let theme = crate::css_fixture::source(
            FileId(2),
            "stillwind.css",
            "@theme { --color-accent: red; --color-muted: gray; }",
        )
        .unwrap();
        let result =
            Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);
        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
        let replacement = &result.transforms[0].replacement;
        assert!(replacement.contains("(n() > 0)&&"), "{replacement}");
        assert!(replacement.contains("!(n() > 0)&&"), "{replacement}");
        assert!(replacement.contains(if frontend == "solid" {
            "{class:"
        } else {
            "{className:"
        }));
    }
}

#[test]
fn plans_react_props_for_classes_a_conditional_and_an_aria_attribute() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60", { "aria-busy": loading });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

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
fn keeps_external_classes_in_an_explicit_clsx_class_entry() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", { class: ["legacy-button", loading && "is-loading", { "legacy-enabled": loading, "legacy-disabled": false }], "aria-busy": loading, "&:hover": "bg-muted" });"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; --color-muted: gray; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.transforms[0].replacement.contains("stillwindClsx("));
    assert!(
        result.transforms[0].replacement.contains("legacy-button"),
        "{}",
        result.transforms[0].replacement
    );
    assert!(
        result.transforms[0].replacement.contains("legacy-enabled"),
        "{}",
        result.transforms[0].replacement
    );
    assert!(
        result.transforms[0]
            .replacement
            .contains("\"aria-busy\":loading")
    );
    assert!(result.css.contains(":hover"));
}

#[test]
fn rejects_clsx_object_form_outside_the_explicit_class_entry() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style({ "legacy-loading": loading });"#,
    )
    .unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "src/styles/stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(
        result.diagnostics.iter().any(|item| item.code == "SHJS003"),
        "{:#?}",
        result.diagnostics
    );
}
