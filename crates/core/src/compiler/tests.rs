use stillwind_source::{FileId, Span};

use super::*;
use super::{declaration_order::combine_declarations, motion::resolve_motion_options};

#[test]
fn expands_authored_variants_through_the_shared_registry() {
    let source = SourceFile::new(
        FileId(1),
        "stillwind.css",
        r#"
                @import "tailwindcss/theme" layer(theme);
                @import "tailwindcss/utilities" layer(utilities);
                .button {
                  @variant hover { opacity: 0.8; }
                  @variant focus { outline-width: 2px; }
                  @variant print { color: black; }
                }
            "#,
    );
    let theme = Compiler::read_theme(&source).unwrap();
    let expanded = expand_authored_directives(theme.custom_css(), &theme);
    let compact = expanded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();

    assert!(
        compact.contains("&:hover{@media(hover:hover){opacity:0.8;}}"),
        "{compact}"
    );
    assert!(compact.contains("&:focus{outline-width:2px;}"), "{compact}");
    assert!(compact.contains("@mediaprint{color:black;}"), "{compact}");
}

#[test]
fn expands_authored_apply_in_builtin_registry_order() {
    let source = SourceFile::new(
        FileId(1),
        "stillwind.css",
        r#"
                @import "tailwindcss/theme" layer(theme);
                @import "tailwindcss/utilities" layer(utilities);
                .button { @apply px-4 bg-red-500 inline-flex; }
            "#,
    );
    let theme = Compiler::read_theme(&source).unwrap();
    let expanded = expand_authored_directives(theme.custom_css(), &theme);
    let compact = expanded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();

    assert!(compact.contains(
            "display:inline-flex;padding-inline:calc(var(--spacing)*4);background-color:var(--color-red-500);"
        ), "actual output: {compact}");
}

#[test]
fn resolves_default_theme_aliases_without_font_name_special_cases() {
    let source = SourceFile::new(
        FileId(1),
        "stillwind.css",
        r#"
                @import "tailwindcss/theme" layer(theme);
                @import "tailwindcss/preflight" layer(base);
                @theme { --font-sans--font-feature-settings: "cv02"; }
            "#,
    );
    let css = Compiler::render_react_stylesheet(&source, "", &CompilerOptions::default()).unwrap();

    assert!(
        css.contains("--default-font-feature-settings:var(--font-sans--font-feature-settings);")
    );
    assert!(css.contains("--font-sans--font-feature-settings:\"cv02\";"));

    let without_feature = SourceFile::new(
        FileId(2),
        "stillwind.css",
        r#"
                @import "tailwindcss/theme" layer(theme);
                @import "tailwindcss/preflight" layer(base);
            "#,
    );
    let css = Compiler::render_react_stylesheet(&without_feature, "", &CompilerOptions::default())
        .unwrap();
    assert!(!css.contains("--default-font-feature-settings:"));
}

#[test]
fn resolves_themed_duration_as_an_animation_option_only() {
    let source = SourceFile::new(
        FileId(1),
        "stillwind.css",
        "@theme { --duration-normal: 180ms; }",
    );
    let theme = Compiler::read_theme(&source).unwrap();
    let options = Spanned::new(
        "duration-normal both".to_owned(),
        Span::new(FileId(2), 0, 20),
    );
    let mut diagnostics = Vec::new();

    assert_eq!(
        resolve_motion_options(&options, &theme, &mut diagnostics),
        "animation-duration:var(--duration-normal);animation-fill-mode:both;"
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn exposes_the_first_frontends_through_one_compiler_api() {
    let js = SourceFile::new(FileId(1), "App.tsx", "export const App = () => <main />;");
    let css = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    );

    assert!(Compiler::check_javascript(&js).is_empty());
    let theme = Compiler::read_theme(&css).unwrap();
    assert_eq!(theme.value("color", "accent"), Some("red"));
    assert!(theme.token("color", "red-500").is_some());
}

#[test]
fn generates_types_from_the_css_theme_vocabulary() {
    let css = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --spacing-4: 1rem; }",
    );
    let types = Compiler::generate_react_types(&css).unwrap();

    assert!(types.contains("\"bg-accent\": true"));
    assert!(types.contains("\"px-4\": true"));
}

#[test]
fn emits_only_used_compiler_owned_property_registrations() {
    let theme = SourceFile::new(
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
    );

    let css = Compiler::render_react_stylesheet(&theme, "", &CompilerOptions::default()).unwrap();
    assert!(css.contains("@property --example-used"));
    assert!(!css.contains("--example-unused"));
    assert!(!css.contains("@stillwind-property"));
}

#[test]
fn preserves_authored_property_rules_over_compiler_defaults() {
    let theme = SourceFile::new(
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
    );

    let css = Compiler::render_react_stylesheet(&theme, "", &CompilerOptions::default()).unwrap();
    assert_eq!(css.matches("@property --tw-shadow").count(), 1);
    assert!(css.contains("inherits: true"));
    assert!(!css.contains("initial-value: 0 0 #0000"));
}

#[test]
fn filters_and_rewrites_builtin_properties_for_generated_utility_css() {
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");
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
fn orders_conflicting_shorthands_like_tailwind() {
    let declarations = [
        CompiledUtility {
            variants: Vec::new(),
            declaration: "margin-left:0.5rem;".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: Some(2),
            candidate: "ml-2".to_owned(),
        },
        CompiledUtility {
            variants: Vec::new(),
            declaration: "margin-inline:0.75rem;".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: Some(1),
            candidate: "mx-3".to_owned(),
        },
        CompiledUtility {
            variants: Vec::new(),
            declaration: "margin:1rem;".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: Some(0),
            candidate: "m-4".to_owned(),
        },
    ];

    let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
    assert_eq!(
        actual,
        "margin:1rem;margin-inline:0.75rem;margin-left:0.5rem;"
    );
}

#[test]
fn keeps_composed_properties_at_their_cascade_position() {
    let declarations = [
        CompiledUtility {
            variants: Vec::new(),
            declaration: "all:unset;".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: Some(usize::MAX),
            candidate: "[all:unset]".to_owned(),
        },
        CompiledUtility {
            variants: Vec::new(),
            declaration: "--tw-skew-x:skewX(6deg);transform:var(--tw-skew-x,);".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: Some(1),
            candidate: "skew-x-6".to_owned(),
        },
    ];

    let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
    assert_eq!(
        actual,
        "--tw-skew-x:skewX(6deg);transform:var(--tw-skew-x,);all:unset;"
    );
}

#[test]
fn keeps_unrelated_declarations_stable() {
    let declarations = [
        CompiledUtility {
            variants: Vec::new(),
            declaration: "color:red;display:block;".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: None,
            candidate: String::new(),
        },
        CompiledUtility {
            variants: Vec::new(),
            declaration: "opacity:0.5;".to_owned(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: None,
            candidate: String::new(),
        },
    ];

    let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
    assert_eq!(actual, "color:red;display:block;opacity:0.5;");
}

#[test]
fn makes_margin_utility_order_independent() {
    let source = SourceFile::new(
        FileId(1),
        "App.tsx",
        r#"import { style } from "stillwind/react"; export const props = style("ml-2 mx-3 m-4");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-2: 0.5rem; --spacing-3: 0.75rem; --spacing-4: 1rem; }",
    );
    let result = Compiler::compile_react(&source, &theme);
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
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-2: 0.5rem; --spacing-3: 0.75rem; --spacing-4: 1rem; }",
    );
    let result = Compiler::compile_react(&source, &theme);
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
