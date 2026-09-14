//! Suppressible macro contracts preserve editable output and JavaScript scope.

use stillwind_compiler::Compiler;
use stillwind_diagnostics::LintKind;
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_source::{FileId, SourceFile};

#[test]
fn suspending_value_expressions_stay_in_the_original_scope() {
    for framework in ["react", "solid", "vue", "svelte", "astro"] {
        for value in ["await read()", "yield read()", "import('module')"] {
            let source = SourceFile::new(FileId(1), "entry.tsx", format!(
                "import {{style}} from 'stillwind/{framework}'; async function* render() {{ return style({{class: {value}, '--x': ({value}), 'data-x': ({value})}}, (await ready()) && {{'data-y': true}}); }}"
            )).unwrap();
            let css =
                SourceFile::new(FileId(2), "theme.css", "@stillwind-slot utilities;").unwrap();
            let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &css);
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert!(
                result
                    .lint_findings
                    .iter()
                    .any(|finding| finding.kind == LintKind::MacroExecutionBoundary)
            );
            assert_eq!(result.lint_policy["macro-execution-boundary"], "deny");
            let code = result.apply_to_source(&source).unwrap();
            let generated = SourceFile::new(FileId(3), "generated.tsx", code.as_str()).unwrap();
            assert!(
                parse(&generated).diagnostics.is_empty(),
                "{framework}: {code}"
            );
        }
    }
}

#[test]
fn contract_severity_and_suppression_leave_transforms_available() {
    for level in ["deny", "warn", "off"] {
        for suppressed in [false, true] {
            let source = SourceFile::new(FileId(1), "entry.tsx", format!(
                "import {{style}} from 'stillwind/react';\n{}const x = style({{'--x': count++}});",
                if suppressed { "// swlint-disable-next-line macro-argument-mutation\n" } else { "" }
            )).unwrap();
            let css = SourceFile::new(
                FileId(2),
                "theme.css",
                format!(
                    "@stillwind {{ macro-argument-mutation: {level}; }} @stillwind-slot utilities;"
                ),
            )
            .unwrap();
            let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &css);
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert!(!result.transforms.is_empty());
            assert_eq!(result.lint_policy["macro-argument-mutation"], level);
            assert_eq!(result.lint_findings.is_empty(), suppressed);
        }
    }
}
