//! Checks composition ordering and provider contributions.

use super::*;

#[test]
fn composition_keeps_distinct_calls_and_merges_selected_bases() {
    let source = SourceFile::new(FileId(1), None, None, "import {style} from 'stillwind/react'; const a=style('m-10 hover:m-20'); const b=style(a,'m-4'); const c=style(b,'m-10'); const d=style(flag?a:c); const e=style(a,c);").unwrap();
    let theme = css_source(FileId(2), "").unwrap();
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
        transformed.contains(
            "stillwindMerge((flag) ? (stillwindToFragment((a))) : (stillwindToFragment((c))))"
        ),
        "{transformed}"
    );
    assert!(
        transformed.contains("stillwindMerge(stillwindToFragment((a)),stillwindToFragment((c)))"),
        "{transformed}"
    );
}

#[test]
fn composition_rejects_props_and_nested_calls() {
    let theme = css_source(FileId(2), "").unwrap();
    for (code, expected) in [
        (
            "const a=style({'data-x':1});const b=style(a,'flex');",
            CompileDiagnostic::StyleBaseWithProps,
        ),
        (
            "const a=style(style('flex'),'block');",
            CompileDiagnostic::NestedStyleCall,
        ),
        (
            "const a=style(b,'flex');const b=style(a,'block');",
            CompileDiagnostic::CompositionCycle,
        ),
    ] {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("import {{style}} from 'stillwind/react'; {code}"),
        )
        .unwrap();
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
        assert!(
            output
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == expected),
            "{:?}",
            output.diagnostics
        );
    }
}

#[test]
fn static_provider_does_not_attach_a_marker_named_like_an_object_property() {
    #[derive(Debug)]
    struct PrototypeGenerator;

    impl IdentifierGenerator for PrototypeGenerator {
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

    let theme = css_source(FileId(2), "").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        "test.css".into(),
        CompilerOptions::default().design_system(),
    )
    .unwrap();
    let source = Arc::new(
        SourceFile::new(
            FileId(1), None, None,
            "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.asAnchor());",
        )
        .unwrap(),
    );
    let scan = OxcSourceAnalyzer.scan(
        Arc::clone(&source),
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("facts"),
        facts: Vec::new(),
        assets: BTreeMap::new(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    let identifiers = Identifiers::new(Some(Rc::new(PrototypeGenerator)), IdentifierMode::Release);
    let compiled =
        ModuleCompiler::new(snapshot, CompilerOptions::default()).compile(&analysis, &identifiers);
    assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
    let replacement = compiled
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("{c:"))
        .unwrap_or_else(|| panic!("{:?}", compiled.code_transforms));
    assert_eq!(replacement, "{c:\"__proto__\"}", "{replacement}");
    let projection = compiled
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("{className:"))
        .unwrap_or_else(|| panic!("{:?}", compiled.code_transforms));
    assert!(!projection.contains("\"__proto__\""), "{projection}");
}
