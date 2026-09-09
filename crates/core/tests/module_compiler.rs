use std::sync::Arc;

use stillwind_core::{
    Compiler, CompilerOptions, DesignSystemSnapshot, ModuleCompiler, OxcSourceAnalyzer,
    SourceAnalyzer,
};
use stillwind_source::{FileId, SourceFile};

#[test]
fn structured_artifact_preserves_css_and_transforms_against_pinned_snapshot() {
    let theme = SourceFile::new(FileId(2), "theme", "@theme { --color-brand: red; }");
    let snapshot = DesignSystemSnapshot::from_source(&theme, CompilerOptions::default()).unwrap();
    for candidates in [
        "flex p-4 hover:bg-brand",
        "before:content-['}']",
        "animate-spin",
        "translate-x-2 border-2 ring-2",
        "md:hover:bg-brand",
    ] {
        let source = Arc::new(SourceFile::new(
            FileId(1),
            "button.tsx",
            format!("import {{ style }} from 'stillwind/react'; const x = style({candidates:?});"),
        ));
        let legacy = Compiler::compile_react(&source, &theme);
        let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
        let compiled = ModuleCompiler::new(snapshot.clone()).compile(&analysis);
        assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
        assert_eq!(compiled.css.to_css(), legacy.module_css, "{candidates}");
        assert_eq!(compiled.to_legacy(&snapshot), legacy, "{candidates}");
        assert_eq!(
            compiled.apply_to_source(&source).unwrap(),
            legacy.apply_to_source(&source).unwrap()
        );
    }
}

#[test]
fn lint_policy_does_not_change_generated_artifacts_and_compilers_are_thread_safe() {
    let source = Arc::new(SourceFile::new(
        FileId(1),
        "a.tsx",
        "import { style } from 'stillwind/react'; const x = style('flex unknown-example');",
    ));
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
    let compile = |policy: &str| {
        let snapshot = DesignSystemSnapshot::from_source(
            &SourceFile::new(
                FileId(2),
                "theme",
                format!("@stillwind {{ unknown-utility: {policy}; }}"),
            ),
            CompilerOptions::default(),
        )
        .unwrap();
        let module = analysis.clone();
        std::thread::spawn(move || ModuleCompiler::new(snapshot).compile(&module))
    };
    let off = compile("off");
    let deny = compile("deny");
    let off = off.join().unwrap();
    let deny = deny.join().unwrap();
    assert_eq!(off.css, deny.css);
    assert_eq!(off.code_transforms, deny.code_transforms);
    assert_eq!(off.symbols, deny.symbols);
    assert!(!deny.lint_findings.is_empty());
}

#[test]
fn invalid_transform_byte_boundaries_return_errors() {
    let source = SourceFile::new(FileId(1), "a.tsx", "日本語");
    let result = stillwind_core::CompileResult {
        transforms: vec![stillwind_core::PropsTransform {
            call_span: stillwind_source::Span::new(FileId(1), 1, 3),
            replacement: "x".into(),
        }],
        ..stillwind_core::CompileResult::default()
    };
    assert!(result.apply_to_source(&source).is_err());
}
