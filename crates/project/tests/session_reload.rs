//! Design-system reloads: a failed reload keeps the last good revision in watch mode
//! and clears artifacts in build mode, and the same split holds for a failed update.

mod support;

use support::{module, snapshot};

use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, Identifiers};
use stillwind_design_system::DesignSystemLoadError;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic};
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{DependencyIndex, ProjectSession, SessionMode};
use stillwind_source::{FileId, ModuleId, SourceFile, SourceRegistry};

/// One analyzed module whose analysis carries a fatal diagnostic.
fn failing_module(name: &str) -> ModuleAnalysis {
    let mut analysis = module(name, "flex");
    analysis.diagnostics.push(CompileError::new(
        CompileDiagnostic::InvalidSymbolicFacts,
        analysis.source.full_span(),
    ));
    analysis
}

fn reload_module() -> ModuleAnalysis {
    let source = SourceFile::new(
        FileId(1), None, None,
        "import { style } from 'stillwind/react'; const motion = style.keyframes({ from: 'brand' }); const x = style(motion('both')); ",
    ).unwrap();
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(source),
            ModuleId::from("motion.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    )
}

#[test]
fn watch_reload_publishes_recovered_motion_and_keeps_other_modules() {
    let mut session = ProjectSession::new(
        snapshot("@utility brand { color:red; }"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    assert_eq!(
        session.update_module(reload_module()).diagnostics,
        [] as [CompileError; 0]
    );
    let other = SourceFile::new(
        FileId(3),
        None,
        None,
        "import { style } from 'stillwind/react'; const x = style('flex');",
    )
    .unwrap();
    let analysis = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(other),
            ModuleId::from("other.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    );
    assert_eq!(
        session.update_module(analysis).diagnostics,
        [] as [CompileError; 0]
    );
    let other_artifact = session.artifacts()[&ModuleId::from("other.tsx")]
        .css
        .to_css();
    let before_css = session.css().unwrap().stylesheet.css;
    let revision = session.snapshot().revision();
    let before_artifact = session.artifacts()[&ModuleId::from("motion.tsx")]
        .css
        .to_css();
    let update = session
        .reload(Ok(snapshot("")), session.options().clone())
        .unwrap();
    assert!(
        session.artifacts()[&ModuleId::from("motion.tsx")]
            .lint_findings
            .iter()
            .any(|error| matches!(error.diagnostic, LintDiagnostic::UnavailableUtility { .. }))
    );
    assert!(update.css_changed);
    assert_ne!(session.snapshot().revision(), revision);
    assert_ne!(session.css().unwrap().stylesheet.css, before_css);
    assert_ne!(
        session.artifacts()[&ModuleId::from("motion.tsx")]
            .css
            .to_css(),
        before_artifact
    );
    assert_eq!(session.artifacts().len(), 2);
    assert_eq!(
        session.artifacts()[&ModuleId::from("other.tsx")]
            .css
            .to_css(),
        other_artifact
    );
    assert!(!session.dependencies().utilities.contains_key("brand"));
    assert!(
        session
            .reload(
                Ok(snapshot("@utility brand { color:blue; }")),
                session.options().clone()
            )
            .unwrap()
            .css_changed
    );
    // A failed module update keeps the watch artifact and its dependency index.
    let dependencies = session.dependencies().clone();
    let update = session.update_module(failing_module("motion.tsx"));
    assert_ne!(update.diagnostics, [] as [CompileError; 0]);
    assert!(
        session
            .artifacts()
            .contains_key(&ModuleId::from("motion.tsx"))
    );
    assert_eq!(session.dependencies(), &dependencies);
}

#[test]
fn build_load_failure_clears_artifacts_and_dependency_indexes() {
    let mut session = ProjectSession::new(
        snapshot("@utility brand { color:red; }"),
        &SourceRegistry::default(),
        SessionMode::Build,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    assert_eq!(
        session.update_module(reload_module()).diagnostics,
        [] as [CompileError; 0]
    );
    assert!(!session.artifacts().is_empty());
    assert!(!session.dependencies().utilities.is_empty());
    assert!(
        session
            .reload(
                Err(DesignSystemLoadError::RequestIdExhausted),
                session.options().clone()
            )
            .is_err()
    );
    assert!(session.artifacts().is_empty());
    assert_eq!(session.stylesheet().rule_count(), 0);
    let deps = session.dependencies();
    assert!(
        deps.candidates.is_empty()
            && deps.utilities.is_empty()
            && deps.variants.is_empty()
            && deps.properties.is_empty()
            && deps.theme_tokens.is_empty()
            && deps.assets.is_empty()
    );
    assert!(matches!(
        session.load_error(),
        Some(DesignSystemLoadError::RequestIdExhausted)
    ));
    assert!(
        session
            .reload(
                Ok(snapshot("@utility brand { color:blue; }")),
                session.options().clone()
            )
            .unwrap()
            .diagnostics
            .is_empty()
    );
    assert!(session.load_error().is_none());
    assert!(
        session
            .artifacts()
            .contains_key(&ModuleId::from("motion.tsx"))
    );
    // A failed module update in build mode drops the artifact and its memberships.
    let update = session.update_module(failing_module("motion.tsx"));
    assert_ne!(update.diagnostics, [] as [CompileError; 0]);
    assert!(
        !session
            .artifacts()
            .contains_key(&ModuleId::from("motion.tsx"))
    );
    assert_eq!(session.dependencies(), &DependencyIndex::default());
}
