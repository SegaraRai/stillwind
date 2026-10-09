//! Injected generators: the names they own, the families that keep their own prefix,
//! and collisions with names an authored document already uses.

mod support;

use support::{brand_snapshot, module};

use std::rc::Rc;
use std::sync::Arc;

use stillwind_compiler::{
    ClassContext, CompilerOptions, ConditionContext, ConditionHelperName, DeclarationContext,
    GeneratedName, GeneratedSymbolKind, IdentifierGenerator, IdentifierHash, IdentifierMode,
    IdentifierPool, Identifiers, KeyframesName, LayerContext, LayerName, UtilityClass,
};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, SourceRegistry};
use stillwind_test_support::css_source;

/// Names utility classes with a fixed, readable body.
#[derive(Debug)]
struct FixedClassGenerator;

impl IdentifierGenerator for FixedClassGenerator {
    fn utility_class(
        &self,
        context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        Ok(GeneratedName::provided(
            UtilityClass::new(&format!("fixed{}", context.branch.len()))
                .expect("canonical test name"),
        ))
    }
}

#[test]
fn session_generators_name_generated_classes() {
    let mut session = ProjectSession::new(
        brand_snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(Some(Rc::new(FixedClassGenerator)), IdentifierMode::Release),
    );
    assert_eq!(
        session
            .update_module(module("button.tsx", "flex"))
            .diagnostics,
        [] as [CompileError; 0]
    );
    let artifact = &session.artifacts()[&ModuleId::from("button.tsx")];
    let class = artifact
        .symbols
        .iter()
        .find(|symbol| symbol.kind == GeneratedSymbolKind::Class)
        .expect("class symbol");
    // The host body is written as returned, with no built-in class prefix.
    assert_eq!(class.name, "fixed7", "{}", class.name);
    assert!(
        session
            .css()
            .unwrap()
            .stylesheet
            .css
            .contains(class.name.as_str())
    );
}

/// Names the keyframes family with one fixed body so it can collide with authored CSS.
#[derive(Debug)]
struct FixedKeyframesGenerator;

impl IdentifierGenerator for FixedKeyframesGenerator {
    fn keyframes_name(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<KeyframesName>, String> {
        Ok(GeneratedName::provided(
            KeyframesName::new("authored").expect("canonical test name"),
        ))
    }
}

#[test]
fn generated_names_never_reuse_an_authored_name_in_the_same_namespace() {
    let theme = css_source(
        FileId(2),
        // The host body carries no built-in prefix, so the authored name it collides
        // with is the bare one.
        "@theme {} @keyframes authored { to { opacity: 0; } }",
    )
    .unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(
            Some(Rc::new(FixedKeyframesGenerator)),
            IdentifierMode::Release,
        ),
    );
    assert!(session.reserved_names().keyframes.contains("authored"));
    let module = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(
                SourceFile::new(
                    FileId(1), None, None,
                    "import { style } from 'stillwind/react'; export const dialog = style.keyframes({ to: 'opacity-0' }); const x = style(dialog('both'));",
                )
                .unwrap(),
            ),
            ModuleId::from("dialog.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        ),
        None,
    );
    let update = session.update_module(module);
    assert!(
        update.diagnostics.iter().any(|error| matches!(
            error.diagnostic,
            CompileDiagnostic::GeneratedIdentifierCollision { .. }
        )),
        "{:?} {:?}",
        update.diagnostics,
        session.artifacts()[&ModuleId::from("dialog.tsx")].symbols
    );
}

/// Names every conditional helper with one fixed body, so two of them collide.
#[derive(Debug)]
struct FixedHelperGenerator;

impl IdentifierGenerator for FixedHelperGenerator {
    fn condition_helper(
        &self,
        _context: &ConditionContext,
        _hash: &IdentifierHash,
        _text_hash: &IdentifierHash,
    ) -> Result<GeneratedName<ConditionHelperName>, String> {
        Ok(GeneratedName::provided(
            ConditionHelperName::new("shared-if").expect("canonical test name"),
        ))
    }
}

#[test]
fn a_helper_name_is_checked_in_the_custom_property_pool() {
    let mut session = ProjectSession::new(
        brand_snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(Some(Rc::new(FixedHelperGenerator)), IdentifierMode::Release),
    );
    let source = Arc::new(
            SourceFile::new(
                FileId(1), None, None,
                "import { style } from 'stillwind/react'; export const x = style('w-[--if(!--c,20px,10px)]', 'h-[--if(!--d,1px,2px)]');",
            )
            .unwrap(),
        );
    let update = session.update_module(OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    ));
    // Two helpers for different CSS may not share the one name the generator returned.
    assert!(
        update.diagnostics.iter().any(|error| matches!(
            error.diagnostic,
            CompileDiagnostic::GeneratedIdentifierCollision { .. }
        )),
        "{:?}",
        update.diagnostics
    );
}

#[test]
fn merge_barriers_have_an_independent_collision_pool() {
    for (candidates, native, collision) in [
        ("if(--a):flex", false, false),
        ("if(--a):flex if(--b):w-4", true, false),
        ("if(--a):flex if(--b):grid", true, true),
    ] {
        let mut session = ProjectSession::new(
            brand_snapshot("red"),
            &SourceRegistry::default(),
            SessionMode::Watch,
            CompilerOptions {
                use_css_if: native,
                ..CompilerOptions::default()
            },
            Identifiers::new(Some(Rc::new(FixedHelperGenerator)), IdentifierMode::Release),
        );
        let source = Arc::new(SourceFile::new(FileId(1), None, None,
            format!("import {{ style }} from 'stillwind/react'; export const x = style({candidates:?});")).unwrap());
        let update = session.update_module(OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                source,
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        ));
        assert_eq!(
            update.diagnostics.iter().any(|error| matches!(
                error.diagnostic,
                CompileDiagnostic::GeneratedIdentifierCollision { .. }
            )),
            collision,
            "{candidates}: {:?}",
            update.diagnostics
        );
        if !collision {
            assert!(update.diagnostics.is_empty(), "{:?}", update.diagnostics);
        }
        if collision {
            continue;
        }
        let artifact = &session.artifacts()[&ModuleId::from("test.tsx")];
        assert!(
            artifact
                .symbols
                .iter()
                .any(|symbol| symbol.pool == IdentifierPool::RuleBoundary),
            "{:?}",
            artifact.symbols
        );
    }
}

/// Names every anonymous layer with one fixed body, so an authored layer can collide with it.
#[derive(Debug)]
struct FixedLayerGenerator;

impl IdentifierGenerator for FixedLayerGenerator {
    fn layer_name(
        &self,
        _context: &LayerContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<LayerName>, String> {
        Ok(GeneratedName::provided(
            LayerName::new("app-layer").expect("canonical test name"),
        ))
    }
}

#[test]
fn a_layer_name_is_checked_against_the_authored_document() {
    let theme = css_source(
        FileId(2),
        "@layer app-layer-0; @layer { @stillwind-slot utilities; }",
    )
    .unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &theme,
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(Some(Rc::new(FixedLayerGenerator)), IdentifierMode::Release),
    );
    assert_eq!(
        session
            .update_module(module("button.tsx", "flex"))
            .diagnostics,
        [] as [CompileError; 0]
    );
    // The render names the document's first anonymous layer, which the authored layer already owns.
    let failure = session.css().unwrap_err();
    assert!(
        matches!(
            failure.diagnostic,
            CompileDiagnostic::GeneratedIdentifierCollision { .. }
        ),
        "{failure:?}"
    );
}
