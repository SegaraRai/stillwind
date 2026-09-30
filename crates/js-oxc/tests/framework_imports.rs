//! Checks macro import compatibility independently of JSX expressions and module names.

use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic};
use stillwind_ir::{
    AdapterKind, ScriptLanguage, ScriptModuleKind, SourceAnalyzer, SourceFormat, SourceInput,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, ModuleId, SourceFile};

fn input(source: &str, language: ScriptLanguage, format: SourceFormat) -> SourceInput {
    let mut input = SourceInput::normal(
        Arc::new(SourceFile::new(FileId(1), None, None, source).unwrap()),
        ModuleId::from("opaque-module"),
        language,
        ScriptModuleKind::Module,
    );
    input.source_format = format;
    input
}

#[test]
fn original_formats_check_value_macro_imports_even_without_calls_or_jsx() {
    for format in [
        SourceFormat::Script,
        SourceFormat::Jsx,
        SourceFormat::Astro,
        SourceFormat::Svelte,
        SourceFormat::Vue,
    ] {
        for adapter in [
            AdapterKind::React,
            AdapterKind::Preact,
            AdapterKind::Solid,
            AdapterKind::Astro,
            AdapterKind::Svelte,
            AdapterKind::Vue,
        ] {
            let import = adapter.spec().macro_import;
            let source = format!("import {{ style as css }} from '{import}'; export {{ css }};");
            let scan = OxcSourceAnalyzer.scan_input(input(&source, ScriptLanguage::Tsx, format));
            assert_eq!(scan.diagnostics, []);
            assert_eq!(scan.local_ir.adapter, adapter);
            let expected = !format.adapters().is_empty() && !format.adapters().contains(&adapter);
            assert_eq!(
                scan.lint_findings.len(),
                usize::from(expected),
                "{format:?}: {import}"
            );
            if expected {
                let finding = &scan.lint_findings[0];
                assert_eq!(finding.code(), "framework-import-mismatch");
                assert_eq!(
                    scan.source.slice(finding.labels[0].span).unwrap(),
                    format!("'{import}'")
                );
            }
        }
    }
}

#[test]
fn type_only_and_non_macro_imports_do_not_select_an_adapter_or_warn() {
    let source = "import type { style } from 'stillwind/svelte'; import { type StyleProps } from 'stillwind/astro'; import { StyleProps as Props } from 'stillwind/svelte';";
    let scan = OxcSourceAnalyzer.scan_input(input(source, ScriptLanguage::Tsx, SourceFormat::Jsx));
    assert_eq!(scan.diagnostics, []);
    assert_eq!(scan.lint_findings, []);
}

#[test]
fn plain_parser_dialects_supply_defaults_without_reading_module_names() {
    for (language, warning) in [
        (ScriptLanguage::Js, false),
        (ScriptLanguage::Ts, false),
        (ScriptLanguage::Jsx, true),
        (ScriptLanguage::Tsx, true),
    ] {
        let mut source = input(
            "import { style } from 'stillwind/astro';",
            language,
            SourceFormat::Script,
        );
        source.source_format = SourceFormat::script(language);
        let scan = OxcSourceAnalyzer.scan_input(source);
        assert_eq!(scan.diagnostics, []);
        assert_eq!(scan.lint_findings.len(), usize::from(warning));
    }
}

#[test]
fn explicit_adapter_conflicts_and_mixed_macros_remain_fatal() {
    let mut source = input(
        "import { style } from 'stillwind/vue';",
        ScriptLanguage::Tsx,
        SourceFormat::Jsx,
    );
    source.framework = Some(AdapterKind::React);
    let scan = OxcSourceAnalyzer.scan_input(source);
    assert_eq!(
        scan.diagnostics[0].diagnostic,
        CompileDiagnostic::FrameworkConflict
    );
    assert_eq!(scan.lint_findings, []);

    let scan = OxcSourceAnalyzer.scan_input(input(
        "import { style as a } from 'stillwind/react'; import { style as b } from 'stillwind/vue';",
        ScriptLanguage::Tsx,
        SourceFormat::Jsx,
    ));
    assert_eq!(
        scan.diagnostics[0].diagnostic,
        CompileDiagnostic::MixedFrameworks
    );
}

#[test]
fn finalization_without_facts_preserves_one_finding() {
    let scan = OxcSourceAnalyzer.scan_input(input(
        "import { style } from 'stillwind/svelte'; const value = style('flex');",
        ScriptLanguage::Tsx,
        SourceFormat::Jsx,
    ));
    let analysis = OxcSourceAnalyzer.finalize(scan, None);
    assert_eq!(analysis.lint_findings.len(), 1);
    assert!(matches!(
        analysis.lint_findings[0].diagnostic,
        LintDiagnostic::FrameworkImportMismatch { .. }
    ));
}
