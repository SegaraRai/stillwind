use super::*;

#[test]
fn compiles_tailwind_group_peer_and_container_compatibility() {
    let component = SourceFile::new(
        FileId(1),
        "src/Relationships.tsx",
        r#"import { style } from "stillwind/react";
export const group = style("group/menu flex @container/sidebar");
export const peer = style("peer/field");
export const child = style("group-hover/menu:bg-red-500 peer-checked/field:block @sm:grid");"#,
    )
    .unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "stillwind.css", "@import \"stillwind\";").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("\"group/menu\""));
    assert!(transformed.contains("\"peer/field\""));
    assert!(
        result
            .css
            .contains("container-name:sidebar;container-type:inline-size;")
    );
    assert!(result.css.contains(":is(:where(.group\\/menu):hover *)"));
    assert!(
        result
            .css
            .contains(":is(:where(.peer\\/field):checked ~ *)")
    );
    assert!(result.css.contains("@container (width >= 24rem)"));
}

#[test]
fn can_forbid_tailwind_relationship_compatibility_with_a_lint_rule() {
    let component = SourceFile::new(
        FileId(1),
        "src/Relationships.tsx",
        "import { style } from \"stillwind/react\"; const child = style(\"group-hover:flex @sm:grid\");",
    ).unwrap();
    let theme =
        crate::css_fixture::source(FileId(2), "stillwind.css", "@import \"stillwind\";").unwrap();
    let options = stillwind_compiler::CompilerOptions {
        forbid_tailwind_relationships: true,
        ..stillwind_compiler::CompilerOptions::default()
    };

    let result = Compiler::compile_source_with_options(
        &stillwind_js_oxc::OxcSourceAnalyzer,
        &component,
        &theme,
        &options,
    );

    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|finding| finding.rule == "prefer-typed-relationships")
            .count(),
        2
    );
    assert!(!result.css.is_empty());
}

#[test]
fn theme_gate_emits_only_required_default_tokens_and_keyframes() {
    let component = SourceFile::new(
        FileId(1),
        "src/Fixture.tsx",
        "import { style } from \"stillwind/react\"; const props = style(\"p-4 animate-spin\");",
    )
    .unwrap();
    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer,
        &component,
        &crate::css_fixture::source(
            FileId(2),
            "stillwind.css",
            "@import \"stillwind/theme.css\" layer(theme); @import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\";",
        ).unwrap(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("--spacing:0.25rem;"));
    assert!(
        result
            .css
            .contains("--animate-spin:spin 1s linear infinite;")
    );
    assert!(result.css.contains("@keyframes spin {"));
    assert!(!result.css.contains("--color-red-500:"));
    assert!(!result.css.contains("@keyframes pulse {"));
}

#[test]
fn breakpoint_variants_inline_queries_without_emitting_lookup_tokens() {
    let component = SourceFile::new(
        FileId(1),
        "src/Fixture.tsx",
        "import { style } from \"stillwind/react\"; const props = style(\"foo:grid 東京:flex\");",
    )
    .unwrap();
    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer,
        &component,
        &crate::css_fixture::source(
            FileId(2),
            "stillwind.css",
            "@import \"stillwind/utilities.css\" layer(utilities); @import \"stillwind/builtins.css\"; @theme { --breakpoint-foo: 60rem; --breakpoint-東京: 70rem; }",
        ).unwrap(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains("@media (width >= 60rem)"),
        "{}",
        result.css
    );
    assert!(result.css.contains("@media (width >= 70rem)"));
    assert!(!result.css.contains("--breakpoint-foo:"));
    assert!(!result.css.contains("--breakpoint-東京:"));
}

#[test]
fn utility_ir_gate_keeps_fallbacks_and_selector_overloads() {
    let component = SourceFile::new(
        FileId(1),
        "src/Fixture.tsx",
        "import { style } from \"stillwind/react\"; const props = style(\"paint-card\");",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &stillwind_js_oxc::OxcSourceAnalyzer,
        &component,
        &crate::css_fixture::source(
            FileId(2),
            "stillwind.css",
            r#"@import "stillwind/utilities.css" layer(utilities);
@utility paint-* { color: red; color: color(display-p3 1 0 0); }
@utility paint-* { &::before { content: "fallback"; } }"#,
        )
        .unwrap(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let class_name = &result.classes[0].class_name;
    assert!(
        result.css.contains(&format!(
            ".{class_name}{{color: red; color: color(display-p3 1 0 0);}}"
        )),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains(&format!(".{class_name}::before{{content: \"fallback\";}}"))
    );
}

#[test]
fn emits_nested_supports_from_css_utility_definitions() {
    let component = SourceFile::new(
        FileId(1),
        "src/Gradient.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-linear-to-r");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("--tw-gradient-position:to right;"));
    assert!(
        result
            .css
            .contains("@supports (background-image:linear-gradient")
    );
    assert!(
        result
            .css
            .contains("--tw-gradient-position:to right in oklab;")
    );
    assert!(
        result
            .css
            .contains("--tw-gradient-position:to right in oklab;")
    );
}

#[test]
fn emits_nested_media_from_css_utility_definitions() {
    let component = SourceFile::new(
        FileId(1),
        "src/Outline.tsx",
        r#"import { style } from "stillwind/react";
const props = style("outline-hidden");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains(
            "@media (forced-colors:active){outline-offset:2px;outline:2px solid transparent;}"
        ),
        "{}",
        result.css
    );
    assert!(result.css.contains("outline:2px solid transparent;"));
}

#[test]
fn emits_selector_bearing_css_utilities_on_their_target_children() {
    let component = SourceFile::new(
        FileId(1),
        "src/Divider.tsx",
        r#"import { style } from "stillwind/react";
const props = style("divide-x-[2px]");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains(":where(.")
            && result.css.contains(
                "border-inline-end-style:var(--tw-border-style);border-inline-end-width:2px;"
            ),
        "{}",
        result.css
    );
}

#[test]
fn emits_starting_style_variants_as_a_css_wrapper() {
    let component = SourceFile::new(
        FileId(1),
        "src/EnteringPanel.tsx",
        r#"import { style } from "stillwind/react";
const props = style("starting:opacity-0");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@starting-style{._s_"));
    assert!(result.css.contains("{opacity:0%;}"), "{}", result.css);
}
