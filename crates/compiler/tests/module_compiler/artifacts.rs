//! Checks pinned compilation output, invalid IR, and artifact identity.

use super::*;

#[test]
fn structured_artifact_preserves_css_and_transforms_against_pinned_snapshot() {
    let theme = css_source(FileId(2), "@theme { --color-brand: red; }").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        "test.css".into(),
        DesignSystemOptions::default(),
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
                None,
                None,
                format!(
                    "import {{ style }} from 'stillwind/react'; const x = style({candidates:?});"
                ),
            )
            .unwrap(),
        );
        let output = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &source,
            &ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &theme,
            &CssSourceName::from("stillwind.css"),
            &CompilerOptions::default(),
        );
        let analysis = OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                source.clone(),
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        );
        let compiled = ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile(&analysis, &Identifiers::default());
        assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
        assert_eq!(compiled.css.to_css(), output.module_css, "{candidates}");
        let fragment = compiled.to_module_output();
        assert_eq!(fragment.css, output.module_css);
        assert_eq!(fragment.transforms, output.transforms);
        assert_eq!(
            StylesheetRenderer::new(
                &snapshot,
                &CompilerOptions::default(),
                &Identifiers::default()
            )
            .render_compilation(&compiled, source.full_span()),
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
fn document_assembly_errors_are_reported_by_the_renderer_not_fragment_conversion() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import {style} from 'stillwind/react'; export const props=style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source.clone(),
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let options = CompilerOptions::default();
    for (css, expected) in [
        (
            "@utility flex {display:flex;}",
            CompileDiagnostic::MissingUtilitiesSlot,
        ),
        (
            "@utility flex {display:flex;} @stillwind-slot utilities; .broken{color:--theme(missing);}",
            CompileDiagnostic::InvalidThemeReference {
                offset: 15,
                detail: "invalid --theme() property `missing` at byte 0".into(),
            },
        ),
    ] {
        let theme = SourceFile::new(FileId(2), None, None, css).unwrap();
        let snapshot =
            DesignSystemSnapshot::from_source(&theme, "test.css".into(), options.design_system())
                .unwrap();
        let compiled = ModuleCompiler::new(snapshot.clone(), options.clone())
            .compile(&analysis, &Identifiers::default());
        assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
        let fragment = compiled.to_module_output();
        assert_eq!(fragment.diagnostics, [] as [CompileError; 0]);
        assert!(fragment.css.contains("display:flex"));
        assert!(fragment.apply_to_source(&source).is_ok());
        let renderer = StylesheetRenderer::new(&snapshot, &options, &Identifiers::default());
        assert_eq!(
            renderer
                .render_stylesheet(&fragment.css)
                .unwrap_err()
                .diagnostic,
            expected
        );
        let document = renderer.render_compilation(&compiled, source.full_span());
        assert_eq!(document.diagnostics[0].diagnostic, expected);
        assert_eq!(document.diagnostics[0].labels[0].span, source.full_span());
        assert_eq!(document.css, "");
        assert_eq!(document.module_css, "");
        assert_eq!(document.classes, [] as [GeneratedClass; 0]);
        assert_eq!(document.symbols, [] as [GeneratedSymbol; 0]);
        assert_eq!(document.transforms, [] as [SourceTransform; 0]);
        assert!(document.apply_to_source(&source).is_err());
        assert_eq!(
            document,
            Compiler::compile_source(
                &OxcSourceAnalyzer,
                &source,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &theme,
                &CssSourceName::from("stillwind.css"),
                &options
            )
        );
    }
}

#[test]
fn lint_settings_does_not_change_generated_artifacts_and_compilers_are_thread_safe() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; const x = style('flex unknown-example');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let compile = |policy: &str| {
        let theme = css_source(FileId(2), "").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            "test.css".into(),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let module = analysis.clone();
        let policy = policy.to_owned();
        std::thread::spawn(move || {
            ModuleCompiler::new(
                snapshot,
                CompilerOptions {
                    lint: LintOverrides {
                        unknown_utility: Some(LintSeverity::parse(&policy).unwrap()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .compile(&module, &Identifiers::default())
        })
    };
    let off = compile("off");
    let deny = compile("deny");
    let off = off.join().unwrap();
    let deny = deny.join().unwrap();
    assert_eq!(off.css, deny.css);
    assert_eq!(off.code_transforms, deny.code_transforms);
    assert_eq!(off.symbols, deny.symbols);
    assert_ne!(deny.lint_findings, [] as [LintFinding; 0]);
}

#[test]
fn invalid_transform_byte_boundaries_return_errors() {
    let source = SourceFile::new(FileId(1), None, None, "日本語").unwrap();
    let result = CompileResult {
        transforms: vec![SourceTransform {
            call_span: Span::new(FileId(1), 1, 3),
            replacement: "x".into(),
            expressions: Vec::new(),
        }],
        ..CompileResult::default()
    };
    assert!(result.apply_to_source(&source).is_err());
}

#[test]
fn invalid_public_ir_returns_diagnostics_instead_of_panicking() {
    let theme = css_source(FileId(2), "").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        "test.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source.clone(),
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let invalid = Span::new(FileId(1), u32::MAX, u32::MAX);
    for (input, expected) in [
        StyleInput::Class(RuleExpression::Utilities(Spanned::new("x".into(), invalid)).into()),
        StyleInput::Class(ClassExpression::ClassValue {
            span: invalid,
            arguments: vec![invalid],
        }),
    ]
    .into_iter()
    .zip([
        CompileDiagnostic::SourceInvalidSpan {
            span: invalid,
            context: SourceFailureContext::Utility,
        },
        CompileDiagnostic::SourceInvalidSpan {
            span: invalid,
            context: SourceFailureContext::Emission,
        },
    ]) {
        let mut malformed = analysis.clone();
        malformed.ir.style_calls[0].inputs = vec![input];
        let compiled = ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile(&malformed, &Identifiers::default());
        assert!(!compiled.is_success());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == expected)
        );
        assert_eq!(compiled.code_transforms, [] as [SourceTransform; 0]);
        assert_eq!(compiled.css.rules, [] as [GeneratedRule; 0]);
    }
}

#[test]
fn codegen_options_change_artifact_identity_without_changing_the_snapshot() {
    let theme = css_source(FileId(2), "").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        "test.css".into(),
        CompilerOptions::default().design_system(),
    )
    .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let compile = |namespace: &str| {
        ModuleCompiler::new(
            snapshot.clone(),
            CompilerOptions {
                class_namespace: namespace.into(),
                ..CompilerOptions::default()
            },
        )
        .compile(&analysis, &Identifiers::default())
    };
    let first = compile("first");
    let second = compile("second");
    assert!(first.is_success() && second.is_success());
    assert_eq!(first.key().design_system(), second.key().design_system());
    assert_eq!(
        first.key().analysis_revision(),
        second.key().analysis_revision()
    );
    assert_ne!(
        first.key().compiler_options(),
        second.key().compiler_options()
    );
    assert_ne!(first.code_transforms, second.code_transforms);
    assert_ne!(first.css, second.css);
}

#[test]
fn changed_frontend_ir_invalidates_the_artifact_for_the_same_source_revision() {
    let theme = css_source(FileId(2), "").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        "test.css".into(),
        CompilerOptions::default().design_system(),
    )
    .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import { style } from 'stillwind/react'; const x = style('flex');",
        )
        .unwrap(),
    );
    let mut analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    let compiler = ModuleCompiler::new(snapshot, CompilerOptions::default());
    let first = compiler.compile(&analysis, &Identifiers::default());
    let mut upstream_changed = analysis.clone();
    let input = &upstream_changed.input;
    let document = SourceDocument::new(
        input.document().file().clone(),
        input.document().format(),
        Some("{\"version\":3,\"sources\":[\"upstream\"]}".into()),
    );
    let prepared = match input.prepared() {
        PreparedSource::Direct(_) => {
            PreparedSource::Direct(DirectSource::try_new(document, input.syntax()).unwrap())
        }
        PreparedSource::Projected(projected) => PreparedSource::Projected(
            ProjectedSource::try_new(document, projected.projection().clone()).unwrap(),
        ),
    };
    let mut rebuilt = SourceInput::new(input.module_id.clone(), prepared);
    rebuilt.framework = input.framework;
    rebuilt
        .stylesheet_imports
        .clone_from(&input.stylesheet_imports);
    rebuilt
        .module_css_import
        .clone_from(&input.module_css_import);
    upstream_changed.input = rebuilt;
    let upstream_key = compiler.key_for(&upstream_changed);
    assert_eq!(
        first.key().source_revision(),
        upstream_key.source_revision()
    );
    assert_ne!(
        first.key().analysis_revision(),
        upstream_key.analysis_revision()
    );
    let StyleInput::Class(ClassExpression::Rule(rule)) = &mut analysis.ir.style_calls[0].inputs[0]
    else {
        panic!("literal utility")
    };
    let RuleExpression::Utilities(value) = rule.as_mut() else {
        panic!("utility")
    };
    value.value = "grid".into();
    let second = compiler.compile(&analysis, &Identifiers::default());
    assert!(first.is_success() && second.is_success());
    assert_eq!(
        first.key().source_revision(),
        second.key().source_revision()
    );
    assert_eq!(
        first.key().semantic_revision(),
        second.key().semantic_revision()
    );
    assert_ne!(
        first.key().analysis_revision(),
        second.key().analysis_revision()
    );
    assert!(first.css.to_css().contains("display:flex"));
    assert!(second.css.to_css().contains("display:grid"));
}
