use std::sync::Arc;

use stillwind_core::{
    CompileResult, Compiler, CompilerOptions, DesignSystemSnapshot, ModuleCompiler,
    OxcSourceAnalyzer, SourceAnalyzer, SourceTransform,
};
use stillwind_source::{FileId, SourceFile, Span};

#[test]
fn structured_artifact_preserves_css_and_transforms_against_pinned_snapshot() {
    let theme =
        crate::css_fixture::source(FileId(2), "theme", "@theme { --color-brand: red; }").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(&theme, CompilerOptions::default()).unwrap();
    for candidates in [
        "flex p-4 hover:bg-brand",
        "before:content-['}']",
        "animate-spin",
        "translate-x-2 border-2 ring-2",
        "md:hover:bg-brand",
    ] {
        let source = Arc::new(
            SourceFile::new(
                FileId(1),
                "button.tsx",
                format!(
                    "import {{ style }} from 'stillwind/react'; const x = style({candidates:?});"
                ),
            )
            .unwrap(),
        );
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
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "a.tsx",
            "import { style } from 'stillwind/react'; const x = style('flex unknown-example');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
    let compile = |policy: &str| {
        let snapshot = DesignSystemSnapshot::from_source(
            &crate::css_fixture::source(
                FileId(2),
                "theme",
                format!("@stillwind {{ unknown-utility: {policy}; }}"),
            )
            .unwrap(),
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
    let source = SourceFile::new(FileId(1), "a.tsx", "日本語").unwrap();
    let result = CompileResult {
        transforms: vec![SourceTransform {
            call_span: Span::new(FileId(1), 1, 3),
            replacement: "x".into(),
        }],
        ..CompileResult::default()
    };
    assert!(result.apply_to_source(&source).is_err());
}

#[test]
fn invalid_public_ir_returns_diagnostics_instead_of_panicking() {
    use stillwind_ir::{HandleDefinition, StyleInput};
    use stillwind_source::Spanned;
    let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(&theme, CompilerOptions::default()).unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "x.tsx",
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
    let span = source.full_span();
    let handle = HandleDefinition {
        module_path: "x.tsx".into(),
        name: Spanned::new("r".into(), span),
        label: None,
        scope: None,
        call_span: span,
    };
    for input in [
        StyleInput::HandleSelectorUtilityString {
            handle: handle.clone(),
            selector: Spanned::new(":hover".into(), span),
            value: Spanned::new("flex".into(), span),
        },
        StyleInput::ConditionalHandleSelectorUtilityString {
            handle,
            selector: Spanned::new(":hover".into(), span),
            value: Spanned::new("flex".into(), span),
            guard: span,
            when_true: true,
        },
        StyleInput::UtilityString(Spanned::new(
            "x".into(),
            Span::new(FileId(1), u32::MAX, u32::MAX),
        )),
    ] {
        let mut malformed = analysis.clone();
        malformed.ir.style_calls[0].inputs = vec![input];
        let compiled = ModuleCompiler::new(snapshot.clone()).compile(&malformed);
        assert!(!compiled.is_success());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|error| matches!(error.code, "SHREF003" | "SHSPAN001"))
        );
        assert!(compiled.css.rules.is_empty());
        assert!(compiled.code_transforms.is_empty());
    }
}

#[path = "../../test-support/css.rs"]
mod css_fixture;
