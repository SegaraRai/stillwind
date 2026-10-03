//! Artifact replacement, reuse, and dependency bookkeeping across module updates and
//! code-generation reloads.

#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::collections::BTreeSet;
use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, Identifiers, StylesheetPlacement, StylesheetRenderer};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemOptions, DesignSystemSnapshot};
use stillwind_ir::{
    ClassExpression, ModuleAnalysis, RuleExpression, ScriptLanguage, ScriptModuleKind,
    SourceAnalyzer, StyleInput,
};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{DependencyIndex, ProjectSession, SessionMode};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, SourceRegistry, SourceUri};

#[test]
fn css_and_script_sources_share_ids_without_sharing_retention() {
    let uri = SourceUri::parse("untitled:theme.css").unwrap();
    let css = SourceFile::new(FileId(2), Some(uri), Some("theme.css".into()), "@theme {}").unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &css,
        CssSourceName::from("theme.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let css_id = snapshot.sources().sources[0].source.id();
    let mut session = ProjectSession::new(
        snapshot,
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    assert!(session.sources().source(css_id).is_err());
    assert!(session.diagnostic_sources().unwrap().source(css_id).is_ok());
    let script = session
        .sources_mut()
        .register(
            Some(SourceUri::parse("untitled:module.tsx").unwrap()),
            Some("module.tsx".into()),
            "export {};",
        )
        .unwrap();
    assert_ne!(script.id(), css_id);
    assert!(session.sources().source(script.id()).is_ok());
    let report_sources = session.diagnostic_sources().unwrap();
    assert!(report_sources.source(css_id).is_ok());
    assert!(report_sources.source(script.id()).is_ok());
}

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

/// A code-generation option change recompiles artifacts without reloading the
/// design-system snapshot or its revision.
#[test]
fn codegen_reload_replaces_artifacts_without_reloading_definitions() {
    let snapshot = snapshot("red");
    let options = CompilerOptions {
        class_namespace: "first".to_owned(),
        ..CompilerOptions::default()
    };
    let mut session = ProjectSession::new(
        snapshot.clone(),
        &SourceRegistry::default(),
        SessionMode::Watch,
        options.clone(),
        Identifiers::default(),
    );
    session.update_module(module("button.tsx", "flex"));
    let before = session.artifacts()[&ModuleId::from("button.tsx")].clone();
    let next = CompilerOptions {
        class_namespace: "second".to_owned(),
        ..options
    };
    let update = session.reload(Ok(snapshot), next.clone()).unwrap();
    let after = &session.artifacts()[&ModuleId::from("button.tsx")];
    assert!(update.diagnostics.is_empty());
    assert!(update.css_changed);
    assert!(
        update
            .transformed_modules
            .contains(&ModuleId::from("button.tsx"))
    );
    assert_eq!(before.key.design_system, after.key.design_system);
    assert_ne!(before.key.compiler_options, after.key.compiler_options);
    assert_ne!(before.code_transforms, after.code_transforms);
    assert_eq!(session.options(), &next);
    let plan = session
        .plan_stylesheet(session.observed_selection(), StylesheetPlacement::Carrier)
        .unwrap();
    assert_eq!(plan.producers[&ModuleId::from("button.tsx")], after.key);
}

/// A carrier render carries the support a module needs and never the document rules.
#[test]
fn module_stylesheets_include_support_without_repeating_document_rules() {
    let snapshot = DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(
            FileId(2),
            "@theme { --color-brand: red; } .document-only{color:blue;}",
        )
        .unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let base = StylesheetRenderer::new(
        &snapshot,
        &CompilerOptions::default(),
        &Identifiers::default(),
    )
    .render_stylesheet("")
    .unwrap();
    assert!(base.starts_with("@layer properties, theme, base, components, utilities;"));
    let mut session = ProjectSession::new(
        snapshot.clone(),
        &SourceRegistry::default(),
        SessionMode::Build,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    session.update_module(module("a.tsx", "bg-brand animate-spin ring-2"));
    let artifact = session.artifacts()[&ModuleId::from("a.tsx")].clone();
    let css = StylesheetRenderer::new(
        &snapshot,
        &CompilerOptions::default(),
        &Identifiers::default(),
    )
    .render_mapped_stylesheet(&artifact.css.mapped(), StylesheetPlacement::Carrier)
    .map(|output| output.css)
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
        StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default()
        )
        .render_stylesheet("")
        .unwrap(),
        base
    );
    assert_eq!(
        StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default()
        )
        .render_mapped_stylesheet(&artifact.css.mapped(), StylesheetPlacement::Carrier)
        .map(|output| output.css)
        .unwrap(),
        css
    );
}

/// A shared rule counts once per distinct payload; removing the last producer removes
/// the occurrence, and a failed reload keeps the last good stylesheet.
#[test]
fn stylesheet_reference_counts_rules_and_preserves_last_good_reload() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
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
    assert!(session.remove_module(&ModuleId::from("a.tsx")));
    assert!(!session.remove_module(&ModuleId::from("a.tsx")));
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
    let analysis_revision = session.analyses()[&ModuleId::from("b.tsx")]
        .source_revision
        .clone();
    let update = session
        .reload(Ok(snapshot("blue")), session.options().clone())
        .unwrap();
    assert!(update.css_changed);
    assert!(update.transformed_modules.is_empty());
    assert_eq!(
        session.analyses()[&ModuleId::from("b.tsx")].source_revision,
        analysis_revision
    );
    assert_ne!(session.snapshot().revision(), pinned.revision());
    let before_removal = session.css().unwrap();
    assert!(session.remove_module(&ModuleId::from("b.tsx")));
    assert_ne!(session.css().unwrap(), before_removal);
    assert_eq!(session.stylesheet().rule_count(), 0);
}

/// Re-publishing identical content reuses the artifact, and removing the module clears
/// every membership it contributed.
#[test]
fn unchanged_artifacts_are_reused_and_dependencies_are_removed() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    session.update_module(module("a.tsx", "bg-brand"));
    let rules = session.artifacts()[&ModuleId::from("a.tsx")]
        .css
        .rules
        .as_ptr();
    assert!(
        !session
            .update_module(module("a.tsx", "bg-brand"))
            .transformed
    );
    assert_eq!(
        rules,
        session.artifacts()[&ModuleId::from("a.tsx")]
            .css
            .rules
            .as_ptr()
    );
    assert!(session.dependencies().candidates["bg-brand"].contains(&ModuleId::from("a.tsx")));
    assert!(
        session.dependencies().theme_tokens["--color-brand"].contains(&ModuleId::from("a.tsx"))
    );
    session.remove_module(&ModuleId::from("a.tsx"));
    assert!(session.dependencies().candidates.is_empty());
    assert!(session.dependencies().theme_tokens.is_empty());
}

/// Replacing a producer retracts exactly its own memberships, and rebuilding the same
/// analyses from scratch reproduces both the indexes and the CSS.
#[test]
fn dependency_replacement_retains_other_producers_and_clears_last_memberships() {
    let snapshot = snapshot("red");
    let mut session = ProjectSession::new(
        snapshot.clone(),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    for name in ["a.tsx", "b.tsx"] {
        let source = Arc::new(SourceFile::new(FileId(1), None, None,
            "import {style} from 'stillwind/react'; const handle=style.ref(); const props=style('hover:bg-brand ring-2');"
        ).unwrap());
        let analysis = OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                source,
                ModuleId::from(name),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        );
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
        BTreeSet::from([ModuleId::from("b.tsx")])
    );
    assert_eq!(
        session.dependencies().candidates["flex"],
        BTreeSet::from([ModuleId::from("a.tsx")])
    );
    let mut rebuilt = ProjectSession::new(
        snapshot,
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    for analysis in session.analyses().values() {
        rebuilt.update_module(analysis.as_ref().clone());
    }
    assert_eq!(session.dependencies(), rebuilt.dependencies());
    assert_eq!(session.css().unwrap(), rebuilt.css().unwrap());

    assert!(session.remove_module(&ModuleId::from("b.tsx")));
    assert_eq!(session.dependencies().assets.len(), 1);
    assert!(session.dependencies().theme_tokens.is_empty());
    assert!(
        !session
            .dependencies()
            .candidates
            .contains_key("hover:bg-brand")
    );
    assert!(session.remove_module(&ModuleId::from("a.tsx")));
    assert_eq!(session.dependencies(), &DependencyIndex::default());
}

#[test]
fn editing_a_session_clone_keeps_the_original_revision_and_dependency_view() {
    let mut original = ProjectSession::new(
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    let id = ModuleId::from("a.tsx");
    original.update_module(module("a.tsx", "bg-brand"));
    let css = original.css().unwrap();
    let dependencies = original.dependencies().clone();
    let mut edited = original.clone();
    assert!(
        edited
            .update_module(module("a.tsx", "flex"))
            .diagnostics
            .is_empty()
    );
    assert_ne!(edited.css().unwrap(), css);
    assert!(edited.dependencies().theme_tokens.is_empty());
    assert_eq!(original.css().unwrap(), css);
    assert_eq!(original.dependencies(), &dependencies);
    assert!(original.current_artifact(&id).is_some());
    assert!(edited.remove_module(&id));
    assert!(edited.current_artifact(&id).is_none());
    assert!(original.current_artifact(&id).is_some());
}

#[test]
fn changed_frontend_inputs_invalidate_an_artifact_with_unchanged_source() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    let mut analysis = module("a.tsx", "flex");
    session.update_module(analysis.clone());
    let before = session
        .current_artifact(&analysis.module_id)
        .unwrap()
        .key
        .clone();
    let StyleInput::Class(ClassExpression::Rule(rule)) = &mut analysis.ir.style_calls[0].inputs[0]
    else {
        panic!("expected a literal utility");
    };
    let RuleExpression::Utilities(value) = rule.as_mut() else {
        panic!("expected utility input");
    };
    value.value = "grid".into();
    assert!(
        session
            .update_module(analysis.clone())
            .diagnostics
            .is_empty()
    );
    let after = session.current_artifact(&analysis.module_id).unwrap();
    assert_ne!(after.key, before);
    assert_eq!(after.key.source_revision, before.source_revision);
    assert!(after.css.to_css().contains("display:grid"));
}
