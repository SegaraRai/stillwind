use std::sync::Arc;

use stillwind_core::{
    CompilerOptions, DesignSystemLoadError, DesignSystemSnapshot, ModuleAnalysis,
    OxcSourceAnalyzer, ProjectSession, SessionMode, SourceAnalyzer,
};
use stillwind_source::{CompileError, FileId, SourceFile};

fn snapshot(color: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &SourceFile::new(
            FileId(2),
            "theme",
            format!("@theme {{ --color-brand: {color}; }}"),
        )
        .unwrap(),
        CompilerOptions::default(),
    )
    .unwrap()
}

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

#[test]
fn stylesheet_reference_counts_rules_and_preserves_last_good_reload() {
    let mut session = ProjectSession::new(snapshot("red"), SessionMode::Watch);
    let pinned = session.snapshot();
    assert!(
        session
            .update_module(module("a.tsx", "bg-brand hover:flex"))
            .unwrap()
            .css_changed
    );
    let count = session.stylesheet().rule_count();
    assert!(
        !session
            .update_module(module("b.tsx", "bg-brand hover:flex"))
            .unwrap()
            .css_changed
    );
    assert_eq!(session.stylesheet().rule_count(), count);
    assert!(!session.remove_module("a.tsx").unwrap().css_changed);
    assert_eq!(session.stylesheet().rule_count(), count);
    let old_css = session.css().unwrap();
    assert!(
        session
            .reload(Err(DesignSystemLoadError::RequestIdExhausted))
            .is_err()
    );
    assert_eq!(session.css().unwrap(), old_css);
    assert_eq!(session.snapshot().revision(), pinned.revision());
    let analysis_revision = session.analyses()["b.tsx"].source_revision.clone();
    let update = session.reload(Ok(snapshot("blue"))).unwrap();
    assert!(update.css_changed);
    assert!(update.transformed_modules.is_empty());
    assert_eq!(
        session.analyses()["b.tsx"].source_revision,
        analysis_revision
    );
    assert_ne!(session.snapshot().revision(), pinned.revision());
    assert!(session.remove_module("b.tsx").unwrap().css_changed);
    assert_eq!(session.stylesheet().rule_count(), 0);
}

#[test]
fn failed_source_update_preserves_watch_artifact_but_not_build_artifact() {
    for mode in [SessionMode::Watch, SessionMode::Build] {
        let mut session = ProjectSession::new(snapshot("red"), mode);
        session.update_module(module("a.tsx", "flex")).unwrap();
        let mut invalid = module("a.tsx", "block");
        invalid.diagnostics.push(CompileError::new(
            "TEST",
            "invalid",
            invalid.source.full_span(),
        ));
        let update = session.update_module(invalid).unwrap();
        assert!(!update.diagnostics.is_empty());
        assert_eq!(
            session.artifacts().contains_key("a.tsx"),
            mode == SessionMode::Watch
        );
    }
}

#[test]
fn unchanged_artifacts_are_reused_and_dependencies_are_removed() {
    let mut session = ProjectSession::new(snapshot("red"), SessionMode::Watch);
    session.update_module(module("a.tsx", "bg-brand")).unwrap();
    let rules = session.artifacts()["a.tsx"].css.rules.as_ptr();
    assert!(
        !session
            .update_module(module("a.tsx", "bg-brand"))
            .unwrap()
            .css_changed
    );
    assert_eq!(rules, session.artifacts()["a.tsx"].css.rules.as_ptr());
    assert!(session.dependencies().candidates["bg-brand"].contains("a.tsx"));
    assert!(session.dependencies().theme_tokens["--color-brand"].contains("a.tsx"));
    session.remove_module("a.tsx").unwrap();
    assert!(session.dependencies().candidates.is_empty());
    assert!(session.dependencies().theme_tokens.is_empty());
}
