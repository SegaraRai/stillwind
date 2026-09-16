//! Module compilation through the public API.
//!
//! Pins composition ordering and the merging of selected bases, artifact identity (an options or
//! frontend-IR change is a different key for the same source revision), structured CSS and
//! transform output against a pinned snapshot, invalid IR and byte boundaries returning
//! diagnostics instead of panicking, and that a lint policy never changes generated artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_compiler::{
    ClassContext, CompileResult, Compiler, CompilerOptions, ConditionContext, CustomPropertyName,
    DeclarationContext, GeneratedName, IdentifierGenerator, IdentifierHash, IdentifierKind,
    IdentifierMode, Identifiers, ModuleCompiler, RefMarker, SourceTransform, StylesheetRenderer,
    UtilityClass,
};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileErrorKind, SourceFailureContext};
use stillwind_ir::{
    ClassExpression, ConditionalKind, HandleDefinition, SourceAnalyzer, StyleInput, StyleVariant,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::TypeFacts;
use stillwind_source::{FileId, Fingerprint, SourceFile, Span, Spanned};

#[test]
fn composition_keeps_distinct_calls_and_merges_selected_bases() {
    let source = SourceFile::new(FileId(1), "composition.ts", "import {style} from 'stillwind/react'; const a=style('m-10 hover:m-20'); const b=style(a,'m-4'); const c=style(b,'m-10'); const d=style(flag?a:c); const e=style(a,c);").unwrap();
    let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
    let output = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.classes.len(), 3);
    let names: BTreeSet<_> = output
        .classes
        .iter()
        .map(|class| &class.class_name)
        .collect();
    assert_eq!(names.len(), 3);
    let transformed = output.apply_to_source(&source).unwrap();
    assert!(
        transformed.contains("stillwindMerge(stillwindToFragment((a)),"),
        "{transformed}"
    );
    assert!(
        transformed.contains("stillwindMerge(stillwindToFragment((flag?a:c)))"),
        "{transformed}"
    );
    assert!(
        transformed.contains("stillwindMerge(stillwindToFragment((a)),stillwindToFragment((c)))"),
        "{transformed}"
    );
}

#[test]
fn composition_rejects_props_and_nested_calls() {
    let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
    for (code, expected) in [
        (
            "const a=style({'data-x':1});const b=style(a,'flex');",
            CompileErrorKind::StyleBaseWithProps,
        ),
        (
            "const a=style(style('flex'),'block');",
            CompileErrorKind::NestedStyleCall,
        ),
        (
            "const a=style(b,'flex');const b=style(a,'block');",
            CompileErrorKind::CompositionCycle,
        ),
    ] {
        let source = SourceFile::new(
            FileId(1),
            "composition.ts",
            format!("import {{style}} from 'stillwind/react'; {code}"),
        )
        .unwrap();
        let output = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|error| error.kind == expected),
            "{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn structured_artifact_preserves_css_and_transforms_against_pinned_snapshot() {
    let theme =
        crate::css_fixture::source(FileId(2), "theme", "@theme { --color-brand: red; }").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
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
        let output = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
        let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
        let compiled =
            ModuleCompiler::new(snapshot.clone(), CompilerOptions::default()).compile(&analysis);
        assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
        assert_eq!(compiled.css.to_css(), output.module_css, "{candidates}");
        let fragment = compiled.to_module_output(&snapshot);
        assert_eq!(fragment.css, output.module_css);
        assert_eq!(fragment.transforms, output.transforms);
        assert_eq!(
            StylesheetRenderer::new(
                &snapshot,
                &CompilerOptions::default(),
                &Identifiers::builtin()
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
            "button.tsx",
            "import {style} from 'stillwind/react'; export const props=style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
    let options = CompilerOptions::default();
    for (css, expected) in [
        (
            "@utility flex {display:flex;}",
            CompileErrorKind::MissingUtilitiesSlot,
        ),
        (
            "@utility flex {display:flex;} @stillwind-slot utilities; .broken{color:--theme(--missing);}",
            CompileErrorKind::UnresolvedThemeReference,
        ),
    ] {
        let theme = SourceFile::new(FileId(2), "root.css", css).unwrap();
        let snapshot = DesignSystemSnapshot::from_source(&theme, options.design_system()).unwrap();
        let compiled = ModuleCompiler::new(snapshot.clone(), options.clone()).compile(&analysis);
        assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
        let fragment = compiled.to_module_output(&snapshot);
        assert!(fragment.diagnostics.is_empty());
        assert!(fragment.css.contains("display:flex"));
        assert!(fragment.apply_to_source(&source).is_ok());
        let renderer = StylesheetRenderer::new(&snapshot, &options, &Identifiers::builtin());
        assert_eq!(
            renderer.render_stylesheet(&fragment.css).unwrap_err(),
            expected
        );
        let document = renderer.render_compilation(&compiled, source.full_span());
        assert_eq!(document.diagnostics[0].kind, expected);
        assert_eq!(document.diagnostics[0].labels[0].span, source.full_span());
        assert!(document.css.is_empty());
        assert!(document.module_css.is_empty());
        assert!(document.classes.is_empty());
        assert!(document.symbols.is_empty());
        assert!(document.transforms.is_empty());
        assert!(document.apply_to_source(&source).is_err());
        assert_eq!(
            document,
            Compiler::compile_source_with_options(&OxcSourceAnalyzer, &source, &theme, &options)
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
        let theme = crate::css_fixture::source(
            FileId(2),
            "theme",
            format!("@stillwind {{ unknown-utility: {policy}; }}"),
        )
        .unwrap();
        let snapshot =
            DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
        let module = analysis.clone();
        std::thread::spawn(move || {
            ModuleCompiler::new(snapshot, CompilerOptions::default()).compile(&module)
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
            expressions: Vec::new(),
        }],
        ..CompileResult::default()
    };
    assert!(result.apply_to_source(&source).is_err());
}

#[test]
fn invalid_public_ir_returns_diagnostics_instead_of_panicking() {
    let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
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
    for (input, expected) in [
        StyleInput::Class(ClassExpression::Variant {
            variant: StyleVariant::Ref {
                handles: vec![handle.clone()],
                prefix: Spanned::new(":hover".into(), span),
            },
            inputs: vec![ClassExpression::Utilities(Spanned::new(
                "flex".into(),
                span,
            ))],
        }),
        StyleInput::Class(ClassExpression::Variant {
            variant: StyleVariant::Ref {
                handles: vec![handle],
                prefix: Spanned::new("hover::".into(), span),
            },
            inputs: vec![ClassExpression::Conditional {
                test: span,
                kind: ConditionalKind::LogicalAnd,
                consequent: vec![ClassExpression::Utilities(Spanned::new(
                    "flex".into(),
                    span,
                ))],
                alternate: Vec::new(),
            }],
        }),
        StyleInput::Class(ClassExpression::Utilities(Spanned::new(
            "x".into(),
            Span::new(FileId(1), u32::MAX, u32::MAX),
        ))),
    ]
    .into_iter()
    .zip([
        CompileErrorKind::InvalidVariantPrefix,
        CompileErrorKind::InvalidVariantPrefix,
        CompileErrorKind::SourceInvalidSpan {
            span: Span::new(FileId(1), u32::MAX, u32::MAX),
            context: SourceFailureContext::Utility,
        },
    ]) {
        let mut malformed = analysis.clone();
        malformed.ir.style_calls[0].inputs = vec![input];
        let compiled =
            ModuleCompiler::new(snapshot.clone(), CompilerOptions::default()).compile(&malformed);
        assert!(!compiled.is_success());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|error| error.kind == expected)
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
    let StyleInput::Class(ClassExpression::Utilities(value)) =
        &mut analysis.ir.style_calls[0].inputs[0]
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

#[test]
fn a_host_body_that_an_object_cannot_hold_is_prefixed_in_generated_code() {
    #[derive(Debug)]
    struct PrototypeGenerator;

    impl IdentifierGenerator for PrototypeGenerator {
        // The marker reaches generated JavaScript as the projection table's key.
        fn ref_marker(
            &self,
            _context: &DeclarationContext,
            _hash: &IdentifierHash,
        ) -> Result<GeneratedName<RefMarker>, String> {
            Ok(GeneratedName::provided(
                RefMarker::new("__proto__").expect("canonical test body"),
            ))
        }
    }

    let theme = css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
            .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "menu.tsx",
            "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.anchor);",
        )
        .unwrap(),
    );
    let scan = OxcSourceAnalyzer.scan(Arc::clone(&source));
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("facts"),
        facts: Vec::new(),
        assets: BTreeMap::new(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    let identifiers = Identifiers::with(Arc::new(PrototypeGenerator));
    let compiled = ModuleCompiler::new(snapshot, CompilerOptions::default())
        .compile_with(&analysis, &identifiers);
    assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
    // The marker is written to CSS and to the projection lookup, so both spell it the
    // same way, and the lookup key is one an object literal can hold.
    let replacement = compiled
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("{c:"))
        .unwrap_or_else(|| panic!("{:?}", compiled.code_transforms));
    assert_eq!(replacement, "{c:\"___proto__\"}", "{replacement}");
    let projection = compiled
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.contains("___proto__\""))
        .unwrap_or_else(|| panic!("{:?}", compiled.code_transforms));
    assert!(!projection.contains("\"__proto__\""), "{projection}");
}

#[test]
fn development_mode_imports_the_checked_helper_from_the_dev_module() {
    let theme = css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
            .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "button.tsx",
            "import { style } from 'stillwind/react'; const x = style('flex', flag && 'grid');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
    let compile = |mode: IdentifierMode| {
        ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile_with(&analysis, &Identifiers::builtin().in_mode(mode))
    };
    let development = compile(IdentifierMode::Development);
    assert!(development.is_success(), "{:?}", development.diagnostics);
    let import = development
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("import "))
        .unwrap_or_else(|| panic!("{:?}", development.code_transforms));
    assert!(
        import.contains("stillwindClsxChecked") && import.contains("stillwind/runtime/dev"),
        "{import}"
    );
    let release = compile(IdentifierMode::Release);
    let import = release
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("import "))
        .unwrap_or_else(|| panic!("{:?}", release.code_transforms));
    assert!(
        import.contains("stillwindClsx") && import.contains("stillwind/react/runtime"),
        "{import}"
    );
}

/// Fails the utility class family.
#[derive(Debug)]
struct FailingGenerator;

impl IdentifierGenerator for FailingGenerator {
    fn utility_class(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        Err("no name for this declaration".to_owned())
    }
}

#[test]
fn development_mode_falls_back_to_builtin_names_and_reports_the_generator() {
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
    let compile = |mode: IdentifierMode| {
        let identifiers = Identifiers::with(Arc::new(FailingGenerator)).in_mode(mode);
        ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile_with(&analysis, &identifiers)
    };
    let development = compile(IdentifierMode::Development);
    assert!(development.is_success(), "{:?}", development.diagnostics);
    assert!(
        development
            .rejected_names
            .iter()
            .any(|rejected| rejected.kind == IdentifierKind::UtilityClass),
        "{:?}",
        development.rejected_names
    );
    let class = development
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "class")
        .expect("class symbol");
    assert!(class.name.starts_with("_s_"), "{}", class.name);
    let release = compile(IdentifierMode::Release);
    assert!(!release.is_success());
    assert!(release.symbols.is_empty());
}

/// Names the ref marker family with a fixed, readable body.
#[derive(Debug)]
struct FixedGenerator;

impl IdentifierGenerator for FixedGenerator {
    fn ref_marker(
        &self,
        context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<RefMarker>, String> {
        Ok(GeneratedName::provided(
            RefMarker::new(&format!("fixed{}", context.binding.len()))
                .expect("canonical test body"),
        ))
    }
}

#[test]
fn injected_generators_name_handles_and_keep_unhandled_families_built_in() {
    let theme = css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
            .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "menu.tsx",
            "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.anchor); const y = style('flex');",
        )
        .unwrap(),
    );
    let identifiers = Identifiers::with(Arc::new(FixedGenerator));
    let scan = OxcSourceAnalyzer.scan(Arc::clone(&source));
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("facts"),
        facts: Vec::new(),
        assets: BTreeMap::new(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    let compiled = ModuleCompiler::new(snapshot, CompilerOptions::default())
        .compile_with(&analysis, &identifiers);
    assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
    let marker = compiled
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "ref-marker")
        .unwrap_or_else(|| panic!("{:?} {:?}", compiled.symbols, compiled.diagnostics));
    // A host body is written as returned, with no built-in marker prefix.
    assert_eq!(marker.name, "fixed4", "{}", marker.name);
    let replacement = compiled
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("{c:"))
        .unwrap_or_else(|| panic!("{:?}", compiled.code_transforms));
    assert!(
        replacement.contains(&marker.name),
        "{replacement} does not name {}",
        marker.name
    );
    let utility = compiled
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "class")
        .expect("class symbol");
    assert!(utility.name.starts_with("_s_"), "{}", utility.name);
}

/// Names the utility class family with a body a selector cannot carry unescaped.
#[derive(Debug)]
struct PunctuationClassGenerator;

impl IdentifierGenerator for PunctuationClassGenerator {
    fn utility_class(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        Ok(GeneratedName::provided(
            UtilityClass::new("a:b").expect("canonical test body"),
        ))
    }
}

#[test]
fn a_host_class_token_keeps_its_spelling_in_the_class_attribute() {
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
    let identifiers = Identifiers::with(Arc::new(PunctuationClassGenerator));
    let compiled = ModuleCompiler::new(snapshot, CompilerOptions::default()).compile_with(
        &OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None),
        &identifiers,
    );
    assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
    // The class attribute carries the token itself, and the rule escapes it for the
    // selector grammar, so both name the same class.
    let class = compiled
        .classes
        .first()
        .expect("generated class")
        .class_name
        .clone();
    assert_eq!(class, "a:b", "{class}");
    let css = compiled.css.to_css();
    assert!(css.contains(r".a\:b{"), "{css}");
}

/// Names every conditional helper the compiler asks for.
#[derive(Debug)]
struct RoleSpellingHelperGenerator;

impl IdentifierGenerator for RoleSpellingHelperGenerator {
    fn condition_helper(
        &self,
        context: &ConditionContext,
        _hash: &IdentifierHash,
        text_hash: &IdentifierHash,
    ) -> Result<GeneratedName<CustomPropertyName>, String> {
        // The role is spelled by the generator and the text is named by its own hash, which is
        // the shape a host picks when it wants two roles to share one suffix.
        Ok(GeneratedName::provided(
            CustomPropertyName::new(&format!(
                "app-if-{}-{}",
                context.role.as_str(),
                &text_hash.to_hex()[..8],
            ))
            .expect("canonical test body"),
        ))
    }
}

#[test]
fn a_host_names_every_conditional_helper_it_is_asked_for() {
    let theme = css_fixture::source(FileId(2), "theme", "").unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, CompilerOptions::default().design_system())
            .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "if.tsx",
            "import { style } from 'stillwind/react'; export const x = style('w-[--if(!--c,20px,10px)]');",
        )
        .unwrap(),
    );
    let identifiers = Identifiers::with(Arc::new(RoleSpellingHelperGenerator));
    let compiled = ModuleCompiler::new(snapshot, CompilerOptions::default()).compile_with(
        &OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None),
        &identifiers,
    );
    assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
    // The declaration that defines the helper and the declaration that reads it agree, and both
    // carry the role the host was asked for.
    let css = compiled.css.to_css();
    assert!(css.contains("--app-if-fallback-"), "{css}");
    assert!(css.contains("var(--app-if-fallback-"), "{css}");
    assert!(!css.contains("--sw-if-"), "{css}");
}
