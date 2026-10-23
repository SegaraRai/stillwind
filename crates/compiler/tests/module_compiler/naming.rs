//! Checks development mode and host name generators.

use lean_string::LeanString;

use super::*;

#[test]
fn development_mode_imports_the_checked_helper_from_the_dev_module() {
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
            "import { style } from 'stillwind/react'; const x = style('flex', flag && 'grid'); const choose = style.cva('flex');",
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
    let compile = |mode: IdentifierMode| {
        ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile(&analysis, &Identifiers::new(None, mode))
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
        import.contains("stillwindClsxChecked")
            && import.contains("stillwindCvaChecked as stillwindCva")
            && import.contains("stillwind/runtime/class-name-object-development"),
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
        import.contains("stillwindClsx")
            && import.contains("stillwindCva")
            && import.contains("stillwind/runtime/class-name-object"),
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
    ) -> Result<GeneratedName<UtilityClass>, LeanString> {
        Err(("no name for this declaration".to_owned()).into())
    }
}

#[test]
fn development_mode_falls_back_to_default_names_and_reports_the_generator() {
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
    let compile = |mode: IdentifierMode| {
        let identifiers = Identifiers::new(Some(Rc::new(FailingGenerator)), mode);
        ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile(&analysis, &identifiers)
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
        .find(|symbol| symbol.kind == GeneratedSymbolKind::Class)
        .expect("class symbol");
    assert!(class.name.starts_with("sw_"), "{}", class.name);
    let release = compile(IdentifierMode::Release);
    assert!(!release.is_success());
    assert_eq!(release.symbols, [] as [GeneratedSymbol; 0]);
}

/// Names the ref marker family with a fixed, readable body.
#[derive(Debug)]
struct FixedGenerator;

impl IdentifierGenerator for FixedGenerator {
    fn ref_marker(
        &self,
        context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<RefMarker>, LeanString> {
        Ok(GeneratedName::provided(
            RefMarker::new(&format!("fixed{}", context.binding.len()))
                .expect("canonical test body"),
        ))
    }
}

#[test]
fn injected_generators_name_handles_and_keep_unhandled_families_built_in() {
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
            "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.asAnchor()); const y = style('flex');",
        )
        .unwrap(),
    );
    let identifiers = Identifiers::new(Some(Rc::new(FixedGenerator)), IdentifierMode::Release);
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
    let compiled =
        ModuleCompiler::new(snapshot, CompilerOptions::default()).compile(&analysis, &identifiers);
    assert!(compiled.is_success(), "{:?}", compiled.diagnostics);
    let marker = compiled
        .symbols
        .iter()
        .find(|symbol| symbol.kind == GeneratedSymbolKind::RefMarker)
        .unwrap_or_else(|| panic!("{:?} {:?}", compiled.symbols, compiled.diagnostics));
    // A host body is written as returned, with no default marker prefix.
    assert_eq!(marker.name, "fixed4", "{}", marker.name);
    let replacement = compiled
        .code_transforms
        .iter()
        .map(|transform| transform.replacement.as_str())
        .find(|replacement| replacement.starts_with("{c:"))
        .unwrap_or_else(|| panic!("{:?}", compiled.code_transforms));
    assert!(
        replacement.contains(marker.name.as_str()),
        "{replacement} does not name {}",
        marker.name
    );
    let utility = compiled
        .symbols
        .iter()
        .find(|symbol| symbol.kind == GeneratedSymbolKind::Class)
        .expect("class symbol");
    assert!(utility.name.starts_with("sw_"), "{}", utility.name);
}

/// Names the utility class family with a body a selector cannot carry unescaped.
#[derive(Debug)]
struct PunctuationClassGenerator;

impl IdentifierGenerator for PunctuationClassGenerator {
    fn utility_class(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, LeanString> {
        Ok(GeneratedName::provided(
            UtilityClass::new("a:b").expect("canonical test body"),
        ))
    }
}

#[test]
fn a_host_class_token_keeps_its_spelling_in_the_class_attribute() {
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
    let identifiers = Identifiers::new(
        Some(Rc::new(PunctuationClassGenerator)),
        IdentifierMode::Release,
    );
    let compiled = ModuleCompiler::new(snapshot, CompilerOptions::default()).compile(
        &OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                source,
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        ),
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
    ) -> Result<GeneratedName<ConditionHelperName>, LeanString> {
        // The role is spelled by the generator and the text is named by its own hash, which is
        // the shape a host picks when it wants two roles to share one suffix.
        Ok(GeneratedName::provided(
            ConditionHelperName::new(&format!(
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
            "import { style } from 'stillwind/react'; export const x = style('w-[--if(!--c,20px,10px)]');",
        )
        .unwrap(),
    );
    let identifiers = Identifiers::new(
        Some(Rc::new(RoleSpellingHelperGenerator)),
        IdentifierMode::Release,
    );
    let compiled = ModuleCompiler::new(snapshot, CompilerOptions::default()).compile(
        &OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                source,
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        ),
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
