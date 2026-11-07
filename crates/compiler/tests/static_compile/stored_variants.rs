//! `style.when()` declarations and their uses.
//!
//! Pins that a use means the same as the stored sequence written at that position,
//! that the call becomes `undefined`, that a use may come before its declaration,
//! that invalid declarations report one finding regardless of the number of uses, and
//! that a declaration from another module expands through checker facts.

use std::collections::BTreeMap;

use stillwind_compiler::{CompileResult, CompilerOptions, render_transforms};
use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{AssetKind, ExportedValue, SymbolicValue, TypeFact, TypeFacts};
use stillwind_source::{CssSourceName, Fingerprint, ModuleId};
use stillwind_test_support::css_source;

use super::*;

const PRELUDE: &str = "import { style } from 'stillwind/react'; \
                       const menu = style.ref('menu'); const trigger = style.ref('trigger'); ";

fn compile(text: &str) -> (CompileResult, String) {
    let source = SourceFile::new(FileId(1), None, None, text).unwrap();
    let theme = css_source(FileId(2), "@import 'stillwind';").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let code = render_transforms(&source, &result.transforms).unwrap().code;
    (result, code)
}

/// Compiles `declarations` and `style(args)`, and returns the stylesheet with
/// numbered class names.
fn stylesheet(declarations: &str, args: &str) -> String {
    let (result, _) = compile(&format!(
        "{PRELUDE}{declarations} export const props = style({args});"
    ));
    assert!(
        result.lint_findings.is_empty(),
        "{args}: {:#?}",
        result.lint_findings
    );
    let mut css = result.css.clone();
    for (index, class) in result.classes.iter().enumerate() {
        css = css.replace(class.class_name.as_str(), &format!("test_class_{index}"));
    }
    css
}

fn kinds(findings: &[LintFinding]) -> Vec<LintKind> {
    findings.iter().map(LintFinding::kind).collect()
}

#[test]
fn a_use_means_the_stored_sequence_written_at_that_position() {
    for (declarations, stored, direct) in [
        (
            "const s = style.when('hover:md');",
            r#"{ [s]: "text-red-500" }"#,
            r#"{ "hover:md:": "text-red-500" }"#,
        ),
        (
            "const s = style.when(`focus`);",
            r"`${s}:active:text-red-500 p-4`",
            r#"[{ "focus:": "active:text-red-500" }, "p-4"]"#,
        ),
        (
            "const s = style.when(trigger('hover').in(menu));",
            r"`md:${s}:text-red-500`",
            r#"`md:${trigger("hover").in(menu)}:text-red-500`"#,
        ),
        (
            "const s = style.when(`md:${trigger`hover`}:${menu.at('@sm')}`);",
            r#"{ [s]: "p-4" }"#,
            r#"{ "md:": { [trigger("hover")]: { [menu.at("@sm")]: "p-4" } } }"#,
        ),
        (
            "const s = style.when(trigger('focus')); const t = style.when(`${s}:hover`); \
             const u = style.when(t);",
            r#"{ [u]: "p-4" }"#,
            r#"{ [trigger("focus")]: { "hover:": "p-4" } }"#,
        ),
    ] {
        assert_eq!(
            stylesheet(declarations, stored),
            stylesheet(declarations, direct),
            "{declarations} {stored}"
        );
    }
}

#[test]
fn the_declaration_becomes_undefined() {
    let (result, code) = compile(&format!(
        "{PRELUDE}export const hovered = style.when(trigger('hover')); \
         export const props = style({{ [hovered]: 'p-4' }});"
    ));
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    assert!(code.contains("export const hovered = undefined;"), "{code}");
    assert!(!code.contains("style.when"), "{code}");
}

#[test]
fn a_style_call_may_use_a_declaration_that_follows_it() {
    let (result, _) = compile(&format!(
        "{PRELUDE}export const View = () => style({{ [hovered]: 'p-4' }}); \
         const hovered = style.when('hover');"
    ));
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    assert!(result.css.contains(":hover"), "{}", result.css);
}

#[test]
fn invalid_declarations_report_one_finding() {
    for declarations in [
        "const s = style.when();",
        "const s = style.when('hover', 'focus');",
        "const s = style.when(...['hover']);",
        "const s = style.when('');",
        "const s = style.when(`${'hover'}:`);",
        "const s = style.when(later); const later = style.when('hover');",
    ] {
        let (result, code) = compile(&format!(
            "{PRELUDE}{declarations} export const a = style({{ [s]: 'p-4' }}); \
             export const b = style(`${{s}}:m-2`);"
        ));
        assert_eq!(
            kinds(&result.lint_findings),
            [LintKind::InvalidStoredVariant],
            "{declarations}"
        );
        assert!(!code.contains("style.when"), "{declarations}: {code}");
    }

    // A sequence that is invalid at the use reports the problem at the declaration
    // once, not once for each use.
    let declarations = "const s = style.when('unknown-variant');";
    let (result, _) = compile(&format!(
        "{PRELUDE}{declarations} export const a = style({{ [s]: 'p-4' }}); \
         export const b = style(`${{s}}:m-2`);"
    ));
    assert_eq!(result.lint_findings.len(), 1, "{:#?}", result.lint_findings);
    let start = PRELUDE.len() + "const s = ".len();
    let span = result.lint_findings[0].info.labels[0].span;
    assert!(
        span.start.0 as usize >= start && (span.end.0 as usize) <= start + declarations.len(),
        "{span:?}"
    );
}

#[test]
fn an_imported_declaration_expands_through_checker_facts() {
    let provider_text = "import { style } from 'stillwind/react'; \
                         export const trigger = style.ref('trigger'); \
                         export const hovered = style.when(`md:${trigger('hover')}`);";
    let provider = OxcSourceAnalyzer.scan(
        std::sync::Arc::new(SourceFile::new(FileId(3), None, None, provider_text).unwrap()),
        ModuleId::from("provider.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let assets: BTreeMap<_, _> = provider
        .summary
        .assets
        .iter()
        .map(|asset| (asset.identity.clone(), asset.metadata.clone()))
        .collect();
    let identity = |kind| {
        assets
            .keys()
            .find(|identity| identity.kind() == kind)
            .unwrap()
            .clone()
    };

    let text = "import { style } from 'stillwind/react'; \
                import { hovered } from './provider'; \
                export const props = style({ [hovered]: 'p-4' }, `${hovered}:m-2`);";
    let source = SourceFile::new(FileId(1), None, None, text).unwrap();
    let scan = OxcSourceAnalyzer.scan(
        std::sync::Arc::new(source.clone()),
        ModuleId::from("consumer.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let start = u32::try_from(text.find("hovered]").unwrap()).unwrap();
    let template = u32::try_from(text.find("hovered}").unwrap()).unwrap();
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("facts"),
        facts: [start, template]
            .into_iter()
            .map(|start| TypeFact {
                start,
                end: start + 7,
                value: ExportedValue::StoredVariants(SymbolicValue::exact(identity(
                    AssetKind::StoredVariant,
                )))
                .into(),
            })
            .collect(),
        assets,
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    let snapshot = stillwind_design_system::DesignSystemSnapshot::from_source(
        &css_source(FileId(2), "@import 'stillwind';").unwrap(),
        CssSourceName::from("stillwind.css"),
        CompilerOptions::default().design_system(),
    )
    .unwrap();
    let options = CompilerOptions::default();
    let identifiers = stillwind_compiler::Identifiers::default();
    let module = stillwind_compiler::ModuleCompiler::new(snapshot.clone(), options.clone())
        .compile(&analysis, &identifiers);
    let result = stillwind_compiler::StylesheetRenderer::new(&snapshot, &options, &identifiers)
        .render_compilation(&module, source.full_span());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    let utilities = result.css.split("@layer utilities{").nth(1).unwrap();
    assert!(utilities.contains("@media (width >= 48rem)"), "{utilities}");
    assert!(utilities.contains(":where(.sw_trigger_"), "{utilities}");
    assert!(utilities.contains(":hover"), "{utilities}");
}
