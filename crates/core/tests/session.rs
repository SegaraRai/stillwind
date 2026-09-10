use std::sync::Arc;

use stillwind_core::{
    ClassIdentity, CompilerOptions, DesignSystemLoadError, DesignSystemSnapshot, ModuleAnalysis,
    OxcSourceAnalyzer, ProjectSession, SessionMode, SourceAnalyzer,
};
use stillwind_source::{CompileError, FileId, SourceFile};

fn snapshot(color: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(
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
fn module_stylesheets_include_support_without_repeating_document_rules() {
    let snapshot = DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(
            FileId(2),
            "theme",
            "@theme { --color-brand: red; } .document-only{color:blue;}",
        )
        .unwrap(),
        CompilerOptions::default(),
    )
    .unwrap();
    let base = snapshot.render_base_stylesheet().unwrap();
    assert!(base.starts_with("@layer properties;\n@layer theme, base, components, utilities;"));
    let mut session = ProjectSession::new(snapshot.clone(), SessionMode::Build);
    session
        .update_module(module("a.tsx", "bg-brand animate-spin ring-2"))
        .unwrap();
    let artifact = session.artifacts()["a.tsx"].clone();
    let css = snapshot
        .render_module_stylesheet(&artifact.css.to_css())
        .unwrap();
    assert!(css.contains("--color-brand:red"));
    assert!(css.contains("@keyframes spin"));
    assert!(css.contains("@property --tw-ring"));
    assert!(css.contains("@layer utilities"));
    assert!(!css.contains(".document-only"));
    assert!(!css.contains("box-sizing:border-box"));
    assert!(base.contains(".document-only"));
    session.update_module(module("late.tsx", "flex")).unwrap();
    assert_eq!(snapshot.render_base_stylesheet().unwrap(), base);
    assert_eq!(
        snapshot
            .render_module_stylesheet(&artifact.css.to_css())
            .unwrap(),
        css
    );
}

#[test]
fn module_salt_is_stable_and_content_identity_remains_available() {
    let compile = |identity, namespace: &str, path: &str| {
        let snapshot = DesignSystemSnapshot::from_source(
            &crate::css_fixture::source(FileId(2), "theme", "").unwrap(),
            CompilerOptions {
                class_identity: identity,
                class_namespace: namespace.into(),
                ..CompilerOptions::default()
            },
        )
        .unwrap();
        let mut session = ProjectSession::new(snapshot, SessionMode::Build);
        let analysis = module(path, "flex");
        let id = analysis.module_id.clone();
        session.update_module(analysis).unwrap();
        session.artifacts()[&id].classes[0].class_name.clone()
    };
    let a = compile(ClassIdentity::Module, "app", "src/a.tsx");
    assert_eq!(a, compile(ClassIdentity::Module, "app", "src\\a.tsx"));
    assert_eq!(a, compile(ClassIdentity::Module, "app", "src/a.tsx"));
    assert_ne!(a, compile(ClassIdentity::Module, "app", "src/b.tsx"));
    assert_ne!(a, compile(ClassIdentity::Module, "other", "src/a.tsx"));
    assert_eq!(
        compile(ClassIdentity::Content, "app", "src/a.tsx"),
        compile(ClassIdentity::Content, "app", "src/b.tsx")
    );
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

#[path = "../../test-support/css.rs"]
mod css_fixture;
