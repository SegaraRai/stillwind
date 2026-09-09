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
            compiled.apply_to_source(&source),
            legacy.apply_to_source(&source)
        );
    }
}
