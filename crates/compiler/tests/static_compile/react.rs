//! React adapter behavior.
//!
//! Pins the shape of the generated call site and of the runtime imports: helper names
//! avoid the bindings a module already uses, imports come from the framework-specific
//! runtime module, a residual macro reference fails the whole compilation, and
//! diagnostics, lint directives, conditional branches, and explicit `class` entries
//! behave as documented.

use stillwind_compiler::{CompilerOptions, GeneratedClass, GeneratedSymbol, SourceTransform};
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic};
use stillwind_ir::{MacroImport, ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::{CssAssertions as _, css_source};

use super::*;

#[test]
fn native_runtime_imports_are_binding_safe_and_framework_specific() {
    let theme = css_source(FileId(2), "").unwrap();
    for framework in ["react", "preact", "solid", "vue", "svelte", "astro"] {
        let component = SourceFile::new(FileId(1), None, None, format!(
            "import {{style as sw, type Theme}} from 'stillwind/{framework}';\n\
             const text = 'stillwindCssValue(';\n\
             function render(stillwindClsx: string) {{ return sw('flex', {{class: stillwindClsx_1}}); }}"
        )).unwrap();
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
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let code = result.apply_to_source(&component).unwrap();
        assert!(code.contains(&format!("import {{ stillwindClsx as stillwindClsx_2 }} from \"stillwind/runtime/{framework}\";")), "{code}");
        assert!(code.contains("stillwindClsx_2(\"sw_"), "{code}");
        assert!(code.contains(",stillwindClsx_1)"), "{code}");
        assert!(code.contains("type Theme"), "{code}");
        assert!(
            code.contains("const text = 'stillwindCssValue(';"),
            "{code}"
        );
        assert!(!code.contains("{ stillwindCssValue"), "{code}");
        assert!(!code.contains("style as sw"), "{code}");
        let reparsed = parse(
            &SourceFile::new(FileId(3), None, None, code).unwrap(),
            &ModuleId::from("test.tsx"),
            &[],
            &std::collections::BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert_eq!(reparsed.diagnostics, [] as [CompileError; 0]);
        assert_eq!(reparsed.module.imports.macros, [] as [MacroImport; 0]);
    }
}

#[test]
fn residual_macro_references_fail_the_entire_compilation() {
    let theme = css_source(FileId(2), "").unwrap();
    for suffix in ["const uncompiled = style;", "export {style};"] {
        let component = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                "import {{style}} from 'stillwind/react'; const props = style('flex'); {suffix}"
            ),
        )
        .unwrap();
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
            result.diagnostics.iter().any(|error| matches!(
                error.diagnostic,
                CompileDiagnostic::UntransformedMacroReference { .. }
            )),
            "{:?}",
            result.diagnostics
        );
        assert_eq!(result.transforms, [] as [SourceTransform; 0]);
        assert_eq!(result.classes, [] as [GeneratedClass; 0]);
        assert_eq!(result.symbols, [] as [GeneratedSymbol; 0]);
        assert_eq!(result.css, "");
    }
    let component = SourceFile::new(FileId(1), None, None,
        "import {style} from 'stillwind/react'; const props = style('flex', {'aria-label': style});"
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
    assert!(
        result.diagnostics.iter().any(|error| matches!(
            error.diagnostic,
            CompileDiagnostic::UntransformedMacroReference { .. }
        )),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.transforms, [] as [SourceTransform; 0]);
    assert_eq!(result.css, "");
}

#[test]
fn compiles_a_real_react_style_call_against_a_css_theme() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";

export function SaveButton() {
  return <button {...style("inline-flex items-center rounded-md bg-accent px-4 text-center")}>Save</button>;
}"#,
    ).unwrap();
    let theme = css_source(
        FileId(2),
        "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; --radius-md: .375rem; }",
    )
    .unwrap();

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
    assert_eq!(result.classes.len(), 1);
    assert!(
        result
            .css
            .starts_with("@layer properties, theme, base, components, utilities;")
    );
    assert!(!result.css.contains("@layer properties;"));
    assert!(result.css.contains_css("--color-accent:oklch(.6 .2 250);"));
    assert!(result.css.contains("@layer base"));
    assert!(result.css.contains(&format!(".{}{{display:inline-flex;padding-inline:var(--spacing-4);align-items:center;text-align:center;background-color:var(--color-accent);border-radius:var(--radius-md);}}", result.classes[0].class_name)), "{}", result.css);
}

#[test]
fn reports_an_unknown_token_at_the_utility_not_the_whole_call() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style("bg-accnet");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme { --color-accent: red; }").unwrap();

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
    let finding = result
        .lint_findings
        .first()
        .expect("an unknown utility finding");

    assert!(matches!(
        finding.diagnostic,
        LintDiagnostic::UnknownUtility { .. }
    ));
    assert_eq!(component.slice(finding.labels[0].span).unwrap(), "accnet");
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("bg-accnet"), "{transformed}");
    assert_ne!(result.css, "");
}

#[test]
fn preserves_unparseable_tailwind_candidates_as_lint_findings() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const props = style("bg-[color:red");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@import \"stillwind\";").unwrap();

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
    assert!(matches!(
        result.lint_findings[0].diagnostic,
        LintDiagnostic::InvalidUtility { .. }
    ));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("bg-[color:red"), "{transformed}");
}

#[test]
fn honors_swlint_disable_directives_by_source_location() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
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
    let theme = css_source(FileId(2), "@theme {}").unwrap();

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
    assert_eq!(result.lint_findings.len(), 1);
    assert!(matches!(
        result.lint_findings[0].diagnostic,
        LintDiagnostic::UnknownUtility { .. }
    ));
    let transformed = result.apply_to_source(&component).unwrap();
    assert_eq!(result.classes, [] as [GeneratedClass; 0]);
    assert!(!transformed.contains("className"), "{transformed}");
}

#[test]
fn emits_css_for_a_runtime_conditional_branch_without_evaluating_it() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60");"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme { --color-accent: red; }").unwrap();

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
    assert_eq!(result.classes.len(), 2);
    assert!(
        result
            .css
            .contains_css("background-color:var(--color-accent);")
    );
    assert!(result.css.contains_css("opacity:60%;"), "{}", result.css);
}

#[test]
fn compiles_both_sides_of_a_static_ternary_branch() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const active: boolean;
const props = style(active ? "bg-accent" : "bg-muted");"#,
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@theme { --color-accent: red; --color-muted: gray; }",
    )
    .unwrap();

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
    assert_eq!(result.classes.len(), 2);
    assert!(
        result
            .css
            .contains_css("background-color:var(--color-accent);")
    );
    assert!(
        result
            .css
            .contains_css("background-color:var(--color-muted);")
    );
    let replacement = &result.transforms[0].replacement;
    assert!(replacement.contains("(active) ? ("), "{replacement}");
    assert!(replacement.contains(") : ("), "{replacement}");
    assert_eq!(replacement.matches("active").count(), 1, "{replacement}");
}

#[test]
fn compound_branch_guards_preserve_precedence_for_both_frontends() {
    for frontend in ["react", "solid"] {
        let component = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                r#"import {{ style }} from "stillwind/{frontend}";
const props = style(n() > 0 ? "bg-accent" : "bg-muted");"#
            ),
        )
        .unwrap();
        let theme = css_source(
            FileId(2),
            "@theme { --color-accent: red; --color-muted: gray; }",
        )
        .unwrap();
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
        let replacement = &result.transforms[0].replacement;
        assert!(replacement.contains("(n() > 0) ? ("), "{replacement}");
        assert_eq!(
            replacement.matches("n() > 0").count(),
            1,
            "the ternary test must be evaluated once: {replacement}"
        );
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
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60", { "aria-busy": loading });"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme { --color-accent: red; }").unwrap();

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
    assert_eq!(result.transforms.len(), 2);
    let transform = &result.transforms[0];
    assert_eq!(transform.call_span, result.classes[0].call_span);
    assert_eq!(
        transform.replacement,
        format!(
            "{{className:stillwindClsx(\"{}\",(loading) && (\"{}\")),\"aria-busy\":loading}}",
            result.classes[0].class_name, result.classes[1].class_name,
        ),
    );
}

#[test]
fn keeps_external_classes_in_an_explicit_clsx_class_entry() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", { class: ["legacy-button", loading && "is-loading", { "legacy-enabled": loading, "legacy-disabled": false }], "aria-busy": loading, "hover:": "bg-muted" });"#,
    ).unwrap();
    let theme = css_source(
        FileId(2),
        "@theme { --color-accent: red; --color-muted: gray; }",
    )
    .unwrap();

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
        None,
        None,
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style({ "legacy-loading": loading });"#,
    )
    .unwrap();
    let theme = css_source(FileId(2), "@theme {}").unwrap();

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
            .any(|item| item.diagnostic == LintDiagnostic::InvalidAttributeKey),
        "{:#?}",
        result.diagnostics
    );
}
