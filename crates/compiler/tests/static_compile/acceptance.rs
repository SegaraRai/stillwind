//! Acceptance cases for the compatibility slice.
//!
//! Pins that a broad CSS-first Tailwind candidate set resolves, that gradient and mask
//! utilities emit their expected declarations, that generated names derive from host
//! module identities rather than process-local file ids, and that relationship
//! selectors and conditional ref attachments are only accepted when the available facts
//! prove them.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{CompileErrorKind, LintDiagnostic};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};

use super::*;

#[test]
fn validates_ref_selector_ancestry_when_jsx_placement_is_known() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const trigger = style.ref();
export const Good = () => <div {...style(trigger, trigger.asAnchor())}><span {...style({ [trigger`hover:`]: "text-accent" })} /></div>;
export const Bad = () => <><div {...style(trigger, trigger.asAnchor())} /><span {...style({ [trigger`hover:`]: "text-accent" })} /></>;"#,
    ).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme { --color-accent: red; }").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| matches!(item.diagnostic, LintDiagnostic::MissingRefAncestor { .. }))
    );
}

#[test]
fn resolves_a_broader_css_first_tailwind_compatibility_slice() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const props = style("grid place-items-center size-10 flex-col grow border outline-2 outline-offset-2 outline-accent z-popover position-area-block-end grid-cols-3 text-3xl");"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "@theme { --spacing-10: 2.5rem; --color-accent: red; --z-index-popover: 50; --text-3xl: 1.875rem; }",
    ).unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains_css("place-items:center;"));
    assert!(
        result
            .css
            .contains_css("width:var(--spacing-10);height:var(--spacing-10);")
    );
    assert!(
        result
            .css
            .contains_css("grid-template-columns:repeat(3,minmax(0,1fr));")
    );
    assert!(
        result
            .css
            .contains_css("outline-color:var(--color-accent);")
    );
    assert!(result.css.contains_css("z-index:var(--z-index-popover);"));
}

#[test]
fn emits_tailwind_background_gradient_and_mask_declarations() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const props = style("bg-red-500/50 bg-linear-to-r from-red-500 via-white to-blue-500 mask-linear-from-0 mask-linear-to-100");"#,
    ).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("background-color:color-mix(in srgb"));
    assert!(result.css.contains("background-color:color-mix(in oklab"));
    assert!(result.css.contains_css("--tw-gradient-position:to right;"));
    assert!(
        result
            .css
            .contains_css("--tw-gradient-from:var(--color-red-500);")
    );
    assert!(
        result
            .css
            .contains_css("--tw-gradient-via:var(--color-white);")
    );
    assert!(
        result
            .css
            .contains_css("--tw-gradient-to:var(--color-blue-500);")
    );
    assert!(result.css.contains("--tw-mask-linear-from-position:"));
    assert!(
        result
            .css
            .contains_css("--tw-mask-linear-to-position:calc(var(--spacing) * 100);")
    );
    assert!(result.css.contains("@property --tw-gradient-position {"));
    assert!(result.css.contains("@property --tw-mask-linear {"));
}

#[test]
fn names_symbols_from_host_module_ids_not_process_local_file_ids() {
    let module = r#"import { style } from "stillwind/react";
const trigger = style.ref();
const props = style(trigger, trigger.asAnchor());"#;
    let windows = SourceFile::new(FileId(1), None, None, module).unwrap();
    let unix = SourceFile::new(FileId(1), None, None, module).unwrap();
    let other = SourceFile::new(FileId(1), None, None, module).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let windows_result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &windows,
        &ModuleId::from("src\\Menu.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    let unix_result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &unix,
        &ModuleId::from("src/Menu.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    let other_result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &other,
        &ModuleId::from("src/Other.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );

    assert_ne!(windows_result.symbols[0].name, unix_result.symbols[0].name);
    assert_ne!(windows_result.symbols[0].name, other_result.symbols[0].name);
    assert!(windows_result.symbols[0].identity.contains("src\\Menu.tsx"));
}

#[test]
fn rejects_conditional_ref_attachments_without_typescript_facts() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const selected = enabled ? first : second;
export const props = style(selected);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert_eq!(
        result.diagnostics[0].kind,
        CompileErrorKind::RefFactsRequired
    );
}

#[test]
fn rejects_conditional_ref_projections_without_typescript_facts() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const selected = enabled ? first : second;
export const anchor = style(selected.asAnchor());
export const container = style(selected.asContainer("inline-size"));"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert_eq!(
        result.diagnostics[0].kind,
        CompileErrorKind::RefProjectionFactsRequired
    );
}
