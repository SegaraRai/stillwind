//! Design-system reloads: a failed reload keeps the last good revision in watch mode
//! and clears artifacts in build mode, and the same split holds for a failed update.

#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, Identifiers};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{ModuleAnalysis, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{DependencyIndex, ProjectSession, SessionMode};
use stillwind_source::{FileId, SourceFile};

/// One analyzed module whose single call uses the given candidate.
fn module(name: &str, candidate: &str) -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(Arc::new(
            SourceFile::new(
                FileId(1),
                name,
                format!(
                    "import {{ style }} from 'stillwind/react'; const x = style({candidate:?});"
                ),
            )
            .unwrap(),
        )),
        None,
    )
}

/// One analyzed module whose analysis carries a fatal diagnostic.
fn failing_module(name: &str) -> ModuleAnalysis {
    let mut analysis = module(name, "flex");
    analysis.diagnostics.push(CompileError::new(
        CompileErrorKind::InvalidSymbolicFacts,
        analysis.source.full_span(),
    ));
    analysis
}

fn reload_snapshot(css: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(FileId(2), "theme", css).unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap()
}

fn reload_module() -> ModuleAnalysis {
    let source = SourceFile::new(
        FileId(1),
        "motion.tsx",
        "import { style } from 'stillwind/react'; const motion = style.keyframes({ from: 'brand' }); const x = style(motion('both')); ",
    ).unwrap();
    OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(source)), None)
}

#[test]
fn watch_reload_rejects_the_entire_revision_when_recompilation_fails() {
    let mut session = ProjectSession::new(
        reload_snapshot("@utility brand { color:red; }"),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::builtin(),
    );
    assert!(
        session
            .update_module(reload_module())
            .diagnostics
            .is_empty()
    );
    let other = SourceFile::new(
        FileId(3),
        "other.tsx",
        "import { style } from 'stillwind/react'; const x = style('flex');",
    )
    .unwrap();
    let analysis =
        OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(std::sync::Arc::new(other)), None);
    assert!(session.update_module(analysis).diagnostics.is_empty());
    let other_artifact = session.artifacts()["other.tsx"].css.to_css();
    let before_css = session.css().unwrap();
    let revision = session.snapshot().revision();
    let before_artifact = session.artifacts()["motion.tsx"].css.to_css();
    let update = session
        .reload(Ok(reload_snapshot("")), session.options().clone())
        .unwrap();
    assert!(
        update.diagnostics["motion.tsx"]
            .iter()
            .any(|error| matches!(error.kind, CompileErrorKind::UnavailableUtility { .. }))
    );
    assert!(!update.css_changed);
    assert!(update.transformed_modules.is_empty());
    assert_eq!(session.snapshot().revision(), revision);
    assert_eq!(session.css().unwrap(), before_css);
    assert_eq!(
        session.artifacts()["motion.tsx"].css.to_css(),
        before_artifact
    );
    assert_eq!(session.artifacts().len(), 2);
    assert_eq!(
        session.artifacts()["other.tsx"].css.to_css(),
        other_artifact
    );
    assert!(session.dependencies().utilities.contains_key("brand"));
    assert!(
        session
            .reload(
                Ok(reload_snapshot("@utility brand { color:blue; }")),
                session.options().clone()
            )
            .unwrap()
            .css_changed
    );
    // A failed module update keeps the watch artifact and its dependency index.
    let dependencies = session.dependencies().clone();
    let update = session.update_module(failing_module("motion.tsx"));
    assert!(!update.diagnostics.is_empty());
    assert!(session.artifacts().contains_key("motion.tsx"));
    assert_eq!(session.dependencies(), &dependencies);
}

#[test]
fn build_load_failure_clears_artifacts_and_dependency_indexes() {
    let mut session = ProjectSession::new(
        reload_snapshot("@utility brand { color:red; }"),
        SessionMode::Build,
        CompilerOptions::default(),
        Identifiers::builtin(),
    );
    assert!(
        session
            .update_module(reload_module())
            .diagnostics
            .is_empty()
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
                Ok(reload_snapshot("@utility brand { color:blue; }")),
                session.options().clone()
            )
            .unwrap()
            .diagnostics
            .is_empty()
    );
    assert!(session.load_error().is_none());
    assert!(session.artifacts().contains_key("motion.tsx"));
    // A failed module update in build mode drops the artifact and its memberships.
    let update = session.update_module(failing_module("motion.tsx"));
    assert!(!update.diagnostics.is_empty());
    assert!(!session.artifacts().contains_key("motion.tsx"));
    assert_eq!(session.dependencies(), &DependencyIndex::default());
}
