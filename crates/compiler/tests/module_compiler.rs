use std::sync::Arc;

use stillwind_compiler::{
    CompileResult, Compiler, CompilerOptions, ModuleCompiler, SourceTransform,
};
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile, Span};

#[test]
fn structured_artifact_preserves_css_and_transforms_against_pinned_snapshot() {
    let theme =
        crate::css_fixture::source(FileId(2), "theme", "@theme { --color-brand: red; }").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        stillwind_design_system::DesignSystemOptions::default(),
    )
    .unwrap();
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
        let output =
            Compiler::compile_source(&stillwind_js_oxc::OxcSourceAnalyzer, &source, &theme);
        let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
        let compiled = ModuleCompiler::new(
            snapshot.clone(),
            stillwind_compiler::CompilerOptions::default(),
        )
        .compile(&analysis);
        assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
        assert_eq!(compiled.css.to_css(), output.module_css, "{candidates}");
        assert_eq!(
            compiled.to_output(&snapshot, &stillwind_compiler::CompilerOptions::default()),
            output,
            "{candidates}"
        );
        assert_eq!(
            compiled.apply_to_source(&source).unwrap(),
            output.apply_to_source(&source).unwrap()
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
            stillwind_design_system::DesignSystemOptions::default(),
        )
        .unwrap();
        let module = analysis.clone();
        std::thread::spawn(move || {
            ModuleCompiler::new(snapshot, stillwind_compiler::CompilerOptions::default())
                .compile(&module)
        })
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
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        stillwind_design_system::DesignSystemOptions::default(),
    )
    .unwrap();
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
        let compiled = ModuleCompiler::new(
            snapshot.clone(),
            stillwind_compiler::CompilerOptions::default(),
        )
        .compile(&malformed);
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

#[test]
fn codegen_options_change_artifact_identity_without_changing_the_snapshot() {
    let theme = css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
            .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "button.tsx",
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
    let compile = |namespace: &str| {
        ModuleCompiler::new(
            snapshot.clone(),
            CompilerOptions {
                class_identity: stillwind_compiler::ClassIdentity::Module,
                class_namespace: namespace.into(),
                ..CompilerOptions::default()
            },
        )
        .compile(&analysis)
    };
    let first = compile("first");
    let second = compile("second");
    assert!(first.is_success() && second.is_success());
    assert_eq!(first.key.design_system, second.key.design_system);
    assert_eq!(first.key.analysis_revision, second.key.analysis_revision);
    assert_ne!(first.key.compiler_options, second.key.compiler_options);
    assert_ne!(first.code_transforms, second.code_transforms);
    assert_ne!(first.css, second.css);
}

#[test]
fn changed_frontend_ir_invalidates_the_artifact_for_the_same_source_revision() {
    let theme = css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
            .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "button.tsx",
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap(),
    );
    let mut analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
    let compiler = ModuleCompiler::new(snapshot, CompilerOptions::default());
    let first = compiler.compile(&analysis);
    let mut upstream_changed = analysis.clone();
    upstream_changed.input.upstream_map = Some("{\"version\":3,\"sources\":[\"upstream\"]}".into());
    let upstream_key = compiler.key_for(&upstream_changed);
    assert_eq!(first.key.source_revision, upstream_key.source_revision);
    assert_ne!(first.key.analysis_revision, upstream_key.analysis_revision);
    let stillwind_ir::StyleInput::UtilityString(value) = &mut analysis.ir.style_calls[0].inputs[0]
    else {
        panic!("literal utility")
    };
    value.value = "grid".into();
    let second = compiler.compile(&analysis);
    assert!(first.is_success() && second.is_success());
    assert_eq!(first.key.source_revision, second.key.source_revision);
    assert_eq!(first.key.semantic_revision, second.key.semantic_revision);
    assert_ne!(first.key.analysis_revision, second.key.analysis_revision);
    assert!(first.css.to_css().contains("display:flex"));
    assert!(second.css.to_css().contains("display:grid"));
}

#[path = "../../test-support/css.rs"]
mod css_fixture;
