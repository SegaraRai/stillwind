//! Checks macro import compatibility independently of JSX expressions and module names.

use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic};
use stillwind_ir::{
    AdapterKind, ConverterIdentity, DirectSource, PreparedSource, ProjectedSource, ScriptLanguage,
    ScriptModuleKind, ScriptSyntax, SourceAnalyzer, SourceDocument, SourceFormat, SourceInput,
    SourceProjection,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, ModuleId, ProjectionMap, SourceFile, VolarMapping};

fn input(source: &str, language: ScriptLanguage, format: SourceFormat) -> SourceInput {
    let file = Arc::new(SourceFile::new(FileId(1), None, None, source).unwrap());
    let syntax = ScriptSyntax::new(language, ScriptModuleKind::Module);
    let prepared = if format.projection_adapter().is_some() {
        let code = Arc::new(SourceFile::new(FileId(2), None, None, source).unwrap());
        let projection = SourceProjection::try_new(
            code,
            syntax,
            ProjectionMap::Volar {
                mappings: vec![VolarMapping {
                    source_offsets: vec![0],
                    generated_offsets: vec![0],
                    lengths: vec![u32::try_from(source.encode_utf16().count()).unwrap()],
                    generated_lengths: None,
                }],
            },
            ConverterIdentity {
                name: "test-converter".into(),
                version: "1".into(),
                options_fingerprint: "test".into(),
            },
            FileId(3),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        PreparedSource::Projected(
            ProjectedSource::try_new(SourceDocument::new(file, format, None), projection).unwrap(),
        )
    } else {
        PreparedSource::Direct(
            DirectSource::try_new(SourceDocument::new(file, format, None), syntax).unwrap(),
        )
    };
    SourceInput::new(ModuleId::from("opaque-module"), prepared)
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
                assert_eq!(finding.code(), "mismatched-framework-import");
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
        let source = SourceInput::normal(
            Arc::new(
                SourceFile::new(
                    FileId(1),
                    None,
                    None,
                    "import { style } from 'stillwind/astro';",
                )
                .unwrap(),
            ),
            ModuleId::from("opaque-module"),
            language,
            ScriptModuleKind::Module,
        );
        let scan = OxcSourceAnalyzer.scan_input(source);
        assert_eq!(scan.diagnostics, []);
        assert_eq!(scan.lint_findings.len(), usize::from(warning));
    }
}

#[test]
fn explicit_adapter_conflicts_and_mixed_macros_remain_fatal() {
    for adapter in [AdapterKind::React, AdapterKind::Solid] {
        let mut source = input(
            "import { style } from 'stillwind/vue';",
            ScriptLanguage::Tsx,
            SourceFormat::Jsx,
        );
        source.framework = Some(adapter);
        let scan = OxcSourceAnalyzer.scan_input(source);
        assert_eq!(
            scan.diagnostics[0].diagnostic,
            CompileDiagnostic::FrameworkConstraintConflict
        );
        assert_eq!(scan.lint_findings, []);
    }

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
        LintDiagnostic::MismatchedFrameworkImport { .. }
    ));
}
