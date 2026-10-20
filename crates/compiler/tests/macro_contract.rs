//! Suppressible macro contracts preserve editable output and JavaScript scope.

use stillwind_compiler::{Compiler, CompilerOptions, SourceTransform};
use stillwind_diagnostics::{LintDiagnostic, LintKind, LintOverrides, LintSeverity};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

#[test]
fn suspending_value_expressions_stay_in_the_original_scope() {
    for framework in ["react", "solid", "vue", "svelte", "astro"] {
        for value in ["await read()", "yield read()", "import('module')"] {
            let source = SourceFile::new(FileId(1), None, None, format!(
                "import {{style}} from 'stillwind/{framework}'; async function* render() {{ return style({{class: {value}, '--x': ({value}), '--other': ({value})}}, (await ready()) && {{'--ready': true}}); }}"
            )).unwrap();
            let css = SourceFile::new(FileId(2), None, None, "@stillwind-slot utilities;").unwrap();
            let result = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &source,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &css,
                &CssSourceName::from("stillwind.css"),
                &CompilerOptions::default(),
            );
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert!(
                result
                    .lint_findings
                    .iter()
                    .any(|finding| finding.diagnostic == LintDiagnostic::SuspendingMacroArgument)
            );
            assert_eq!(
                result
                    .lint_settings
                    .reporting_severity(LintKind::SuspendingMacroArgument),
                LintSeverity::Deny
            );
            let code = result.apply_to_source(&source).unwrap();
            let generated = SourceFile::new(FileId(3), None, None, code.as_str()).unwrap();
            assert!(
                parse(
                    &generated,
                    &ModuleId::from("test.tsx"),
                    &[],
                    &std::collections::BTreeMap::default(),
                    ScriptLanguage::Tsx,
                    ScriptModuleKind::Unambiguous
                )
                .diagnostics
                .is_empty(),
                "{framework}: {code}"
            );
        }
    }
}

#[test]
fn contract_severity_and_suppression_leave_transforms_available() {
    for level in ["deny", "warn", "off"] {
        for suppressed in [false, true] {
            let source = SourceFile::new(FileId(1), None, None, format!(
                "import {{style}} from 'stillwind/react';\n{}const x = style({{'--x': count++}});",
                if suppressed { "// swlint-disable-next-line mutating-macro-argument\n" } else { "" }
            )).unwrap();
            let css = SourceFile::new(FileId(2), None, None, "@stillwind-slot utilities;").unwrap();
            let result = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &source,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &css,
                &CssSourceName::from("stillwind.css"),
                &CompilerOptions {
                    lint: LintOverrides {
                        mutating_macro_argument: Some(LintSeverity::parse(level).unwrap()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert_ne!(result.transforms, [] as [SourceTransform; 0]);
            assert_eq!(
                result
                    .lint_settings
                    .reporting_severity(LintKind::MutatingMacroArgument)
                    .as_str(),
                level
            );
            assert_eq!(result.lint_findings.is_empty(), suppressed);
            let serialized = serde_json::to_value(&result).unwrap();
            assert_eq!(serialized["lintSettings"]["mutating-macro-argument"], level);
        }
    }
}
