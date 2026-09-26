//! Injected generators: the names they own, the families that keep their own prefix,
//! and collisions with names an authored document already uses.

#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::sync::Arc;

use stillwind_compiler::{
    ClassContext, CompilerOptions, ConditionContext, ConditionHelperName, DeclarationContext,
    GeneratedName, IdentifierGenerator, IdentifierHash, IdentifierMode, Identifiers, KeyframesName,
    LayerContext, LayerName, UtilityClass,
};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_semantic::ModuleId;
use stillwind_source::{FileId, SourceFile, SourceRegistry};

/// One snapshot whose theme declares a single token.
fn snapshot(color: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(FileId(2), format!("@theme {{ --color-brand: {color}; }}"))
            .unwrap(),
        "test.css",
        DesignSystemOptions::default(),
    )
    .unwrap()
}

/// One analyzed module whose single call uses the given candidate.
fn module(name: &str, candidate: &str) -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(
                SourceFile::new(
                    FileId(1),
                    None,
                    None,
                    format!(
                        "import {{ style }} from 'stillwind/react'; const x = style({candidate:?});"
                    ),
                )
                .unwrap(),
            ),
            ModuleId::from(name),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    )
}

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
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(Some(Arc::new(FixedClassGenerator)), IdentifierMode::Release),
    );
    assert!(
        session
            .update_module(module("button.tsx", "flex"))
            .diagnostics
            .is_empty()
    );
    let artifact = &session.artifacts()["button.tsx"];
    let class = artifact
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "class")
        .expect("class symbol");
    // The host body is written as returned, with no built-in class prefix.
    assert_eq!(class.name, "fixed7", "{}", class.name);
    assert!(session.css().unwrap().contains(&class.name));
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
    let theme = crate::css_fixture::source(
        FileId(2),
        // The host body carries no built-in prefix, so the authored name it collides
        // with is the bare one.
        "@theme {} @keyframes authored { to { opacity: 0; } }",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, "test.css", DesignSystemOptions::default())
            .unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(
            Some(Arc::new(FixedKeyframesGenerator)),
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
            error.kind,
            CompileErrorKind::GeneratedIdentifierCollision { .. }
        )),
        "{:?} {:?}",
        update.diagnostics,
        session.artifacts()["dialog.tsx"].symbols
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
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(
            Some(Arc::new(FixedHelperGenerator)),
            IdentifierMode::Release,
        ),
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
            error.kind,
            CompileErrorKind::GeneratedIdentifierCollision { .. }
        )),
        "{:?}",
        update.diagnostics
    );
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
    let theme = crate::css_fixture::source(
        FileId(2),
        "@layer app-layer-0; @layer { @stillwind-slot utilities; }",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, "test.css", DesignSystemOptions::default())
            .unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(Some(Arc::new(FixedLayerGenerator)), IdentifierMode::Release),
    );
    assert!(
        session
            .update_module(module("button.tsx", "flex"))
            .diagnostics
            .is_empty()
    );
    // The render names the document's first anonymous layer, which the authored layer already owns.
    let failure = session.css().unwrap_err();
    assert!(
        matches!(
            failure,
            CompileErrorKind::GeneratedIdentifierCollision { .. }
        ),
        "{failure:?}"
    );
}
