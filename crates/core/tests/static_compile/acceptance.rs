use super::*;

#[test]
fn validates_ref_selector_ancestry_when_jsx_placement_is_known() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const trigger = style.ref();
export const Good = () => <div {...style(trigger.anchor)}><span {...style({ [trigger`&:hover`]: "text-accent" })} /></div>;
export const Bad = () => <><div {...style(trigger.anchor)} /><span {...style({ [trigger`&:hover`]: "text-accent" })} /></>;"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_react(&component, &theme);

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| item.rule == "valid-ref-ancestry")
    );
}

#[test]
fn resolves_a_broader_css_first_tailwind_compatibility_slice() {
    let component = SourceFile::new(
        FileId(1),
        "src/ModernCard.tsx",
        r#"import { style } from "stillwind/react";
const props = style("grid place-items-center size-10 flex-col grow border outline-2 outline-offset-2 outline-accent z-popover position-area-block-end grid-cols-3 text-3xl");"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-10: 2.5rem; --color-accent: red; --z-index-popover: 50; --text-3xl: 1.875rem; }",
    ).unwrap();

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("place-items:center;"));
    assert!(
        result
            .css
            .contains("width:var(--spacing-10);height:var(--spacing-10);")
    );
    assert!(
        result
            .css
            .contains("grid-template-columns:repeat(3,minmax(0,1fr));")
    );
    assert!(result.css.contains("outline-color:var(--color-accent);"));
    assert!(result.css.contains("z-index:var(--z-index-popover);"));
}

#[test]
fn emits_tailwind_background_gradient_and_mask_declarations() {
    let component = SourceFile::new(
        FileId(1),
        "src/Hero.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-red-500/50 bg-linear-to-r from-red-500 via-white to-blue-500 mask-linear-from-0 mask-linear-to-100");"#,
    ).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("background-color:color-mix(in srgb"));
    assert!(result.css.contains("background-color:color-mix(in oklab"));
    assert!(result.css.contains("--tw-gradient-position:to right;"));
    assert!(
        result
            .css
            .contains("--tw-gradient-from:var(--color-red-500);")
    );
    assert!(result.css.contains("--tw-gradient-via:var(--color-white);"));
    assert!(
        result
            .css
            .contains("--tw-gradient-to:var(--color-blue-500);")
    );
    assert!(result.css.contains("--tw-mask-linear-from-position:"));
    assert!(
        result
            .css
            .contains("--tw-mask-linear-to-position:calc(var(--spacing) * 100);")
    );
    assert!(result.css.contains("@property --tw-gradient-position {"));
    assert!(result.css.contains("@property --tw-mask-linear {"));
}

#[test]
fn names_symbols_from_normalized_module_paths_not_process_local_file_ids() {
    let module = r#"import { style } from "stillwind/react";
const trigger = style.ref();
const props = style(trigger.anchor);"#;
    let windows = SourceFile::new(FileId(1), "src\\Menu.tsx", module).unwrap();
    let unix = SourceFile::new(FileId(1), "src/Menu.tsx", module).unwrap();
    let other = SourceFile::new(FileId(1), "src/Other.tsx", module).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let windows_result = Compiler::compile_react(&windows, &theme);
    let unix_result = Compiler::compile_react(&unix, &theme);
    let other_result = Compiler::compile_react(&other, &theme);

    assert_eq!(windows_result.symbols[0].name, unix_result.symbols[0].name);
    assert_ne!(windows_result.symbols[0].name, other_result.symbols[0].name);
    assert!(windows_result.symbols[0].identity.contains("src/Menu.tsx"));
}

#[test]
fn rejects_conditional_ref_attachments_without_typescript_facts() {
    let component = SourceFile::new(
        FileId(1),
        "src/Dynamic.tsx",
        r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const selected = enabled ? first : second;
export const props = style(selected);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_react(&component, &theme);
    assert_eq!(result.diagnostics[0].code, "SHTS001");
}

#[test]
fn rejects_conditional_ref_projections_without_typescript_facts() {
    let component = SourceFile::new(
        FileId(1),
        "src/DynamicProjection.tsx",
        r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const selected = enabled ? first : second;
export const anchor = style(selected.anchor);
export const container = style(selected.container("inline-size"));"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_react(&component, &theme);
    assert_eq!(result.diagnostics[0].code, "SHTS001");
}
