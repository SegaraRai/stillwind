//! Names an authored document already uses: per-family pools, producer-authored CSS,
//! and collisions reported at publication time.

#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::sync::Arc;

use stillwind_compiler::{
    AnchorName, CompilerOptions, DeclarationContext, GeneratedName, IdentifierGenerator,
    IdentifierHash, IdentifierMode, Identifiers, LocatorContext, RefMarker,
};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, SourceRegistry};

/// One snapshot whose theme declares a single token.
fn snapshot(color: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(FileId(2), format!("@theme {{ --color-brand: {color}; }}"))
            .unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap()
}

/// Names the ref marker and anchor families with one fixed body.
#[derive(Debug)]
struct FixedBodyGenerator;

impl IdentifierGenerator for FixedBodyGenerator {
    fn ref_marker(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<RefMarker>, String> {
        Ok(GeneratedName::provided(
            RefMarker::new("shared").expect("canonical test name"),
        ))
    }

    fn anchor_name(
        &self,
        _context: &LocatorContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<AnchorName>, String> {
        Ok(GeneratedName::provided(
            AnchorName::new("shared").expect("canonical test name"),
        ))
    }
}

/// One module that declares a ref and uses its anchor.
fn ref_module() -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(
                SourceFile::new(
                    FileId(1), None, None,
                    "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.asAnchor());",
                )
                .unwrap(),
            ),
            ModuleId::from("panel.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous
        ),
        None,
    )
}

#[test]
fn distinct_pools_may_publish_the_same_spelling() {
    let generators = Identifiers::new(Some(Arc::new(FixedBodyGenerator)), IdentifierMode::Release);
    let mut session = ProjectSession::new(
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        generators,
    );
    let update = session.update_module(ref_module());
    assert!(update.diagnostics.is_empty(), "{:?}", update.diagnostics);
    let artifact = &session.artifacts()[&ModuleId::from("panel.tsx")];
    let anchor_pool = artifact
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "ref")
        .expect("ref anchor symbol");
    let marker_pool = artifact
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "ref-marker")
        .expect("ref marker symbol");
    assert!(anchor_pool.name.ends_with("shared"));
    assert!(marker_pool.name.ends_with("shared"));
    assert_ne!(anchor_pool.pool, marker_pool.pool);
}

#[test]
fn css_a_producer_authors_is_reserved_when_the_host_supplies_it() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::new(Some(Arc::new(FixedBodyGenerator)), IdentifierMode::Release),
    );
    assert_eq!(
        session.update_module(ref_module()).diagnostics,
        [] as [CompileError; 0]
    );
    // The producer's own stylesheet names its anchor `shared`, which the generator also returned.
    let reported =
        session.reserve_authored_css(&ModuleId::from("panel.css"), ":root{anchor-name:shared;}");
    assert!(
        reported.values().flatten().any(|error| matches!(
            error.diagnostic,
            CompileDiagnostic::GeneratedIdentifierCollision { .. }
        )),
        "{reported:?}"
    );
    // Forgetting the stylesheet gives the name back.
    assert!(
        session
            .forget_authored_css(&ModuleId::from("panel.css"))
            .is_empty()
    );
}

#[test]
fn a_generated_anchor_name_is_checked_against_the_authored_document() {
    let theme = crate::css_fixture::source(
        FileId(2),
        ":root{anchor-name:shared;} @stillwind-slot utilities;",
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
        Identifiers::new(Some(Arc::new(FixedBodyGenerator)), IdentifierMode::Release),
    );
    let update = session.update_module(ref_module());
    // The generator names the anchor `shared`, which the authored declaration already owns.
    assert!(
        update.diagnostics.iter().any(|error| matches!(
            error.diagnostic,
            CompileDiagnostic::GeneratedIdentifierCollision { .. }
        )),
        "{:?}",
        update.diagnostics
    );
}
