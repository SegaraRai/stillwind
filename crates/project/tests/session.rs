use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, StylesheetPlacement, StylesheetRenderer};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemOptions, DesignSystemSnapshot};
use stillwind_ir::{ModuleAnalysis, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{DependencyIndex, ProjectSession, SessionMode};
use stillwind_source::{CompileError, FileId, SourceFile};

fn snapshot(color: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(
            FileId(2),
            "theme",
            format!("@theme {{ --color-brand: {color}; }}"),
        )
        .unwrap(),
        DesignSystemOptions::default(),
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
fn codegen_reload_replaces_artifacts_without_reloading_definitions() {
    let snapshot = snapshot("red");
    let options = CompilerOptions {
        class_namespace: "first".into(),
        ..CompilerOptions::default()
    };
    let mut session = ProjectSession::new(snapshot.clone(), SessionMode::Watch, options.clone());
    session.update_module(module("button.tsx", "flex"));
    let before = session.artifacts()["button.tsx"].clone();
    let next = CompilerOptions {
        class_namespace: "second".into(),
        ..options
    };
    let update = session.reload(Ok(snapshot), next.clone()).unwrap();
    let after = &session.artifacts()["button.tsx"];
    assert!(update.diagnostics.is_empty());
    assert!(update.css_changed);
    assert!(update.transformed_modules.contains("button.tsx"));
    assert_eq!(before.key.design_system, after.key.design_system);
    assert_ne!(before.key.compiler_options, after.key.compiler_options);
    assert_ne!(before.code_transforms, after.code_transforms);
    assert_eq!(session.options(), &next);
    let plan = session
        .plan_stylesheet(session.observed_selection(), StylesheetPlacement::Carrier)
        .unwrap();
    assert_eq!(plan.producers["button.tsx"], after.key);
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
        DesignSystemOptions::default(),
    )
    .unwrap();
    let base = StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
        .render_base_stylesheet()
        .unwrap();
    assert!(base.starts_with("@layer properties;\n@layer theme, base, components, utilities;"));
    let mut session = ProjectSession::new(
        snapshot.clone(),
        SessionMode::Build,
        CompilerOptions::default(),
    );
    session.update_module(module("a.tsx", "bg-brand animate-spin ring-2"));
    let artifact = session.artifacts()["a.tsx"].clone();
    let css = StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
        .render_module_stylesheet(&artifact.css.to_css())
        .unwrap();
    assert!(css.contains("--color-brand:red"));
    assert!(css.contains("@keyframes spin"));
    assert!(css.contains("@property --tw-ring"));
    assert!(css.contains("@layer utilities"));
    assert!(!css.contains(".document-only"));
    assert!(!css.contains("box-sizing:border-box"));
    assert!(base.contains(".document-only"));
    session.update_module(module("late.tsx", "flex"));
    assert_eq!(
        StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
            .render_base_stylesheet()
            .unwrap(),
        base
    );
    assert_eq!(
        StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
            .render_module_stylesheet(&artifact.css.to_css())
            .unwrap(),
        css
    );
}

#[test]
fn call_identity_is_stable_and_module_and_namespace_specific() {
    let compile = |namespace: &str, path: &str| {
        let options = CompilerOptions {
            class_namespace: namespace.into(),
            ..CompilerOptions::default()
        };
        let snapshot = DesignSystemSnapshot::from_source(
            &crate::css_fixture::source(FileId(2), "theme", "").unwrap(),
            options.design_system(),
        )
        .unwrap();
        let mut session = ProjectSession::new(snapshot, SessionMode::Build, options);
        let analysis = module(path, "flex");
        let id = analysis.module_id.clone();
        session.update_module(analysis);
        session.artifacts()[&id].classes[0].class_name.clone()
    };
    let a = compile("app", "src/a.tsx");
    assert_eq!(a, compile("app", "src\\a.tsx"));
    assert_eq!(a, compile("app", "src/a.tsx"));
    assert_ne!(a, compile("app", "src/b.tsx"));
    assert_ne!(a, compile("other", "src/a.tsx"));
}

#[test]
fn stylesheet_reference_counts_rules_and_preserves_last_good_reload() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        SessionMode::Watch,
        CompilerOptions::default(),
    );
    let pinned = session.snapshot();
    let base_css = session.css().unwrap();
    assert!(
        session
            .update_module(module("a.tsx", "bg-brand hover:flex"))
            .transformed
    );
    let shared_css = session.css().unwrap();
    assert_ne!(shared_css, base_css);
    let count = session.stylesheet().rule_count();
    assert!(
        session
            .update_module(module("b.tsx", "bg-brand hover:flex"))
            .transformed
    );
    assert_ne!(session.css().unwrap(), shared_css);
    assert_eq!(session.stylesheet().rule_count(), 2 * count);
    assert!(session.remove_module("a.tsx"));
    assert!(!session.remove_module("a.tsx"));
    assert_ne!(session.css().unwrap(), shared_css);
    assert_eq!(session.stylesheet().rule_count(), count);
    let old_css = session.css().unwrap();
    assert!(
        session
            .reload(
                Err(DesignSystemLoadError::RequestIdExhausted),
                session.options().clone()
            )
            .is_err()
    );
    assert_eq!(session.css().unwrap(), old_css);
    assert_eq!(session.snapshot().revision(), pinned.revision());
    let analysis_revision = session.analyses()["b.tsx"].source_revision.clone();
    let update = session
        .reload(Ok(snapshot("blue")), session.options().clone())
        .unwrap();
    assert!(update.css_changed);
    assert!(update.transformed_modules.is_empty());
    assert_eq!(
        session.analyses()["b.tsx"].source_revision,
        analysis_revision
    );
    assert_ne!(session.snapshot().revision(), pinned.revision());
    let before_removal = session.css().unwrap();
    assert!(session.remove_module("b.tsx"));
    assert_ne!(session.css().unwrap(), before_removal);
    assert_eq!(session.stylesheet().rule_count(), 0);
}

#[test]
fn failed_source_update_preserves_watch_artifact_but_not_build_artifact() {
    for mode in [SessionMode::Watch, SessionMode::Build] {
        let mut session = ProjectSession::new(snapshot("red"), mode, CompilerOptions::default());
        session.update_module(module("a.tsx", "flex"));
        let dependencies = session.dependencies().clone();
        let mut invalid = module("a.tsx", "block");
        invalid.diagnostics.push(CompileError::new(
            "TEST",
            "invalid",
            invalid.source.full_span(),
        ));
        let update = session.update_module(invalid);
        assert!(!update.diagnostics.is_empty());
        assert_eq!(
            session.artifacts().contains_key("a.tsx"),
            mode == SessionMode::Watch
        );
        let expected = if mode == SessionMode::Watch {
            dependencies
        } else {
            DependencyIndex::default()
        };
        assert_eq!(session.dependencies(), &expected);
    }
}

#[test]
fn unchanged_artifacts_are_reused_and_dependencies_are_removed() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        SessionMode::Watch,
        CompilerOptions::default(),
    );
    session.update_module(module("a.tsx", "bg-brand"));
    let rules = session.artifacts()["a.tsx"].css.rules.as_ptr();
    assert!(
        !session
            .update_module(module("a.tsx", "bg-brand"))
            .transformed
    );
    assert_eq!(rules, session.artifacts()["a.tsx"].css.rules.as_ptr());
    assert!(session.dependencies().candidates["bg-brand"].contains("a.tsx"));
    assert!(session.dependencies().theme_tokens["--color-brand"].contains("a.tsx"));
    session.remove_module("a.tsx");
    assert!(session.dependencies().candidates.is_empty());
    assert!(session.dependencies().theme_tokens.is_empty());
}

#[test]
fn dependency_replacement_retains_other_producers_and_clears_last_memberships() {
    let snapshot = snapshot("red");
    let mut session = ProjectSession::new(
        snapshot.clone(),
        SessionMode::Watch,
        CompilerOptions::default(),
    );
    for name in ["a.tsx", "b.tsx"] {
        let source = Arc::new(SourceFile::new(FileId(1), name,
            "import {style} from 'stillwind/react'; const scope=style.scope(); const props=style('hover:bg-brand ring-2');"
        ).unwrap());
        let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None);
        assert!(session.update_module(analysis).diagnostics.is_empty());
    }
    assert_eq!(session.dependencies().assets.len(), 4);
    assert!(!session.dependencies().utilities.is_empty());
    assert!(!session.dependencies().variants.is_empty());
    assert!(!session.dependencies().properties.is_empty());
    assert_eq!(
        session.dependencies().theme_tokens["--color-brand"].len(),
        2
    );

    assert!(
        session
            .update_module(module("a.tsx", "flex"))
            .diagnostics
            .is_empty()
    );
    assert_eq!(session.dependencies().assets.len(), 3);
    assert_eq!(
        session.dependencies().theme_tokens["--color-brand"],
        ["b.tsx".into()].into()
    );
    assert_eq!(
        session.dependencies().candidates["flex"],
        ["a.tsx".into()].into()
    );
    let mut rebuilt = ProjectSession::new(snapshot, SessionMode::Watch, CompilerOptions::default());
    for analysis in session.analyses().values() {
        rebuilt.update_module(analysis.clone());
    }
    assert_eq!(session.dependencies(), rebuilt.dependencies());
    assert_eq!(session.css().unwrap(), rebuilt.css().unwrap());

    assert!(session.remove_module("b.tsx"));
    assert_eq!(session.dependencies().assets.len(), 1);
    assert!(session.dependencies().theme_tokens.is_empty());
    assert!(
        !session
            .dependencies()
            .candidates
            .contains_key("hover:bg-brand")
    );
    assert!(session.remove_module("a.tsx"));
    assert_eq!(session.dependencies(), &DependencyIndex::default());
}

#[path = "../../test-support/css.rs"]
mod css_fixture;
