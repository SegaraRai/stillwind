//! Complete keyframes branch replacement and detached CSS conditions.

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions};
use stillwind_diagnostics::LintDiagnostic;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

fn compile(body: &str) -> CompileResult {
    let source = SourceFile::new(FileId(1), None, None, format!(
        "import {{style}} from 'stillwind/react'; const fade = style.keyframes({{{body}}}); export const x = style(fade(''));"
    )).unwrap();
    let css = SourceFile::new(FileId(2), None, None, "@utility a {opacity:0;} @utility b {opacity:1;} @utility c {opacity:.5;} @variant motion-reduce {@media (prefers-reduced-motion:reduce) {@slot;}} @variant hover {&:hover {@slot;}} @stillwind-slot utilities; @stillwind-slot keyframes;").unwrap();
    Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &css,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    )
}

#[test]
fn branches_replace_all_frames_and_nested_conditions_conjoin() {
    let result = compile(
        "from:'a', to:'b', 'motion-reduce:': {to:'c', '[@supports(display:grid)]:': {from:'b'}}",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(
        result.css.contains("{from{opacity:0;}to{opacity:1;}}"),
        "{}",
        result.css
    );
    assert!(result.css.contains("{to{opacity:.5;}}"), "{}", result.css);
    assert!(
        result
            .css
            .contains("@media (prefers-reduced-motion:reduce){@supports (display:grid){@keyframes"),
        "{}",
        result.css
    );
    assert_eq!(result.css.matches("@keyframes ").count(), 3);
    assert_eq!(result.css.matches("from{opacity:0;}").count(), 1);
}

#[test]
fn sibling_branch_order_uses_the_shared_variant_order() {
    let a = "'motion-reduce:': {to:'c'}";
    let b = "'[@supports(display:grid)]:': {from:'b'}";
    let first = compile(&format!("from:'a', {a}, {b}"));
    let second = compile(&format!("from:'a', {b}, {a}"));
    assert!(first.diagnostics.is_empty(), "{first:#?}");
    assert!(second.diagnostics.is_empty(), "{second:#?}");
    assert_eq!(first.css, second.css);
}

#[test]
fn invalid_keyframe_contexts_omit_the_conditioned_branch() {
    for key in [
        "hover:",
        "[&:hover]:",
        "[.global]:",
        "[@container(width>1px)]:",
        "[@starting-style]:",
    ] {
        let result = compile(&format!("from:'a', '{key}': {{to:'b'}}"));
        assert!(
            result
                .lint_findings
                .iter()
                .any(|error| error.diagnostic == LintDiagnostic::InvalidKeyframesVariant),
            "{key}: {result:#?}"
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.css.contains("from{opacity:0;}"), "{}", result.css);
        assert!(!result.css.contains("to{opacity:1;}"), "{}", result.css);
    }
    let result = compile("from:'motion-reduce:a'");
    assert!(
        result
            .lint_findings
            .iter()
            .any(|error| error.diagnostic == LintDiagnostic::KeyframesFrameVariants)
    );
}
