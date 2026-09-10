//! Integration of frontend/theme loading, registries and final compiler output.

use stillwind_source::{FileId, SourceFile};

use super::Compiler;
use crate::CompilerOptions;

#[test]
fn resolves_default_theme_aliases_without_font_name_special_cases() {
    let source = crate::css_fixture::source(
        FileId(1),
        "stillwind.css",
        r#"
                @import "stillwind/theme.css" layer(theme);
                @import "stillwind/preflight.css" layer(base);
                @theme { --font-sans--font-feature-settings: "cv02"; }
            "#,
    )
    .unwrap();
    let css = Compiler::render_react_stylesheet(&source, "", &CompilerOptions::default()).unwrap();

    assert!(
        css.contains("--default-font-feature-settings:var(--font-sans--font-feature-settings);")
    );
    assert!(css.contains("--font-sans--font-feature-settings:\"cv02\";"));

    let without_feature = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        r#"
                @import "stillwind/theme.css" layer(theme);
                @import "stillwind/preflight.css" layer(base);
            "#,
    )
    .unwrap();
    let css = Compiler::render_react_stylesheet(&without_feature, "", &CompilerOptions::default())
        .unwrap();
    assert!(!css.contains("--default-font-feature-settings:"));
}

#[test]
fn compiles_theme_backed_styles_through_the_source_analyzer() {
    let js = SourceFile::new(
        FileId(1),
        "App.tsx",
        r#"import { style } from "stillwind/react"; export const props = style("bg-accent");"#,
    )
    .unwrap();
    let css = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();
    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &js, &css);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.css.contains("--color-accent:red;"), "{}", result.css);
    assert!(
        result.css.contains("background-color:var(--color-accent)"),
        "{}",
        result.css
    );
    assert_eq!(result.classes.len(), 1);
    let class = &result.classes[0].class_name;
    assert!(result.css.contains(&format!(".{class}")));
    let transformed = result.apply_to_source(&js).unwrap();
    assert!(transformed.contains(class), "{transformed}");
    assert!(transformed.contains("className"), "{transformed}");
    assert!(!transformed.contains("style("), "{transformed}");
    assert_eq!(
        transformed,
        format!(" export const props = {{className:\"{class}\"}};")
    );
}

#[test]
fn emits_only_used_compiler_owned_property_registrations() {
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        r#"
                @stillwind-property --example-used {
                  syntax: "<number>";
                  inherits: false;
                  initial-value: 0;
                }
                @stillwind-property --example-unused {
                  syntax: "<number>";
                  inherits: false;
                  initial-value: 0;
                }
                .example { --example-used: 1; color: var(--example-used); }
            "#,
    )
    .unwrap();

    let css = Compiler::render_react_stylesheet(&theme, "", &CompilerOptions::default()).unwrap();
    assert!(css.contains("@property --example-used"));
    assert!(!css.contains("--example-unused"));
    assert!(!css.contains("@stillwind-property"));
}

#[test]
fn preserves_authored_property_rules_over_compiler_defaults() {
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        r#"
                @property --tw-shadow {
                  syntax: "<color>";
                  inherits: true;
                  initial-value: red;
                }
                .example { color: var(--tw-shadow); }
            "#,
    )
    .unwrap();

    let css = Compiler::render_react_stylesheet(&theme, "", &CompilerOptions::default()).unwrap();
    assert_eq!(css.matches("@property --tw-shadow").count(), 1);
    assert!(css.contains("inherits: true"));
    assert!(!css.contains("initial-value: 0 0 #0000"));
}

#[test]
fn filters_and_rewrites_builtin_properties_for_generated_utility_css() {
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();
    let options = CompilerOptions {
        tailwind_prefix: "--internal-".to_owned(),
        ..CompilerOptions::default()
    };

    let css = Compiler::render_react_stylesheet(
        &theme,
        ".example{--internal-translate-x:0;translate:var(--internal-translate-x);}",
        &options,
    )
    .unwrap();

    assert!(css.contains("@property --internal-translate-x"));
    assert!(css.contains("@property --internal-translate-y"));
    assert!(css.contains("@property --internal-translate-z"));
    assert!(!css.contains("@property --internal-gradient-from"));
}

#[test]
fn makes_margin_utility_order_independent() {
    let source = SourceFile::new(
        FileId(1),
        "App.tsx",
        r#"import { style } from "stillwind/react"; export const props = style("ml-2 mx-3 m-4");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-2: 0.5rem; --spacing-3: 0.75rem; --spacing-4: 1rem; }",
    )
    .unwrap();
    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &source, &theme);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let margin = result.css.find("margin:").expect("margin shorthand");
    let inline = result.css.find("margin-inline:").expect("margin axis");
    let left = result.css.find("margin-left:").expect("margin side");
    assert!(margin < inline && inline < left, "{0}", result.css);
}

#[test]
fn orders_whole_builtin_utilities_instead_of_individual_properties() {
    let source = SourceFile::new(
        FileId(1),
        "App.tsx",
        r#"import { style } from "stillwind/react"; export const props = style("h-2 w-3 size-4");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-2: 0.5rem; --spacing-3: 0.75rem; --spacing-4: 1rem; }",
    )
    .unwrap();
    let result = Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &source, &theme);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let size_width = result
        .css
        .find("width:var(--spacing-4)")
        .expect("size width");
    let size_height = result
        .css
        .find("height:var(--spacing-4)")
        .expect("size height");
    let height = result.css.find("height:var(--spacing-2)").expect("height");
    let width = result.css.find("width:var(--spacing-3)").expect("width");
    assert!(
        size_width < size_height && size_height < height && height < width,
        "{}",
        result.css
    );
}
