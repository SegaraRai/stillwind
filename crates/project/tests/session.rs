//! End-to-end tests for project sessions and stylesheet plans.
//!
//! These drive the public orchestration API with real analyses from the Oxc frontend,
//! so they cover the behaviour a host actually depends on: artifact replacement on
//! reload, last-known-good retention in watch mode versus invalidation in build mode,
//! shared rules surviving removal of one producer, and dependency indexes losing
//! memberships exactly when their last user goes.

use std::sync::Arc;

use stillwind_compiler::{
    AnchorName, ClassContext, CompilerOptions, ConditionContext, CustomPropertyName,
    DeclarationContext, GeneratedName, GeneratedRule, IdentifierGenerator, IdentifierHash,
    Identifiers, KeyframesName, LayerContext, LayerName, LocatorContext, ModuleCssArtifact,
    RefMarker, RuleKey, StylesheetPlacement, StylesheetRenderer, UtilityClass,
};
use stillwind_design_system::{DesignSystemLoadError, DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{ModuleAnalysis, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{DependencyIndex, ProjectSession, ProjectStylesheet, SessionMode};
use stillwind_source::{FileId, Fingerprint, SourceFile};

#[test]
fn publication_rekeys_publicly_edited_rules_before_sharing_them() {
    let key = RuleKey(Fingerprint::of("stale"));
    let mut artifact = ModuleCssArtifact {
        rules: vec![GeneratedRule {
            keyframes: false,
            required_properties: Vec::new(),
            key,
            prelude: ".x".into(),
            body: Some("color:red;".into()),
        }],
    };
    let mut sheet = ProjectStylesheet::default();
    sheet.replace("a".into(), &artifact);
    artifact.rules[0].body = Some("color:blue;".into());
    sheet.replace("b".into(), &artifact);
    assert_eq!(sheet.rule_count(), 2);
    assert!(sheet.module_css().unwrap().contains("color:red;"));
    assert!(sheet.module_css().unwrap().contains("color:blue;"));
    sheet.remove("a");
    assert_eq!(sheet.rule_count(), 1);
    assert!(!sheet.module_css().unwrap().contains("color:red;"));
}

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
    let mut session = ProjectSession::new(
        snapshot.clone(),
        SessionMode::Watch,
        options.clone(),
        Identifiers::builtin(),
    );
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
    let base = StylesheetRenderer::new(
        &snapshot,
        &CompilerOptions::default(),
        &Identifiers::builtin(),
    )
    .render_base_stylesheet()
    .unwrap();
    assert!(base.starts_with("@layer properties, theme, base, components, utilities;"));
    let mut session = ProjectSession::new(
        snapshot.clone(),
        SessionMode::Build,
        CompilerOptions::default(),
        Identifiers::builtin(),
    );
    session.update_module(module("a.tsx", "bg-brand animate-spin ring-2"));
    let artifact = session.artifacts()["a.tsx"].clone();
    let css = StylesheetRenderer::new(
        &snapshot,
        &CompilerOptions::default(),
        &Identifiers::builtin(),
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
            &Identifiers::builtin()
        )
        .render_base_stylesheet()
        .unwrap(),
        base
    );
    assert_eq!(
        StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::builtin()
        )
        .render_mapped_stylesheet(&artifact.css.mapped(), StylesheetPlacement::Carrier)
        .map(|output| output.css)
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
        let mut session = ProjectSession::new(
            snapshot,
            SessionMode::Build,
            options,
            Identifiers::builtin(),
        );
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
        Identifiers::builtin(),
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
        let mut session = ProjectSession::new(
            snapshot("red"),
            mode,
            CompilerOptions::default(),
            Identifiers::builtin(),
        );
        session.update_module(module("a.tsx", "flex"));
        let dependencies = session.dependencies().clone();
        let mut invalid = module("a.tsx", "block");
        invalid.diagnostics.push(CompileError::new(
            CompileErrorKind::InvalidSymbolicFacts,
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
        Identifiers::builtin(),
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
        Identifiers::builtin(),
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
    let mut rebuilt = ProjectSession::new(
        snapshot,
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::builtin(),
    );
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
    let theme = css_fixture::source(
        FileId(2),
        "theme",
        // The host body carries no built-in prefix, so the authored name it collides
        // with is the bare one.
        "@theme {} @keyframes authored { to { opacity: 0; } }",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::with(Arc::new(FixedKeyframesGenerator)),
    );
    assert!(session.reserved_names().keyframes.contains("authored"));
    let module = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(Arc::new(
            SourceFile::new(
                FileId(1),
                "dialog.tsx",
                "import { style } from 'stillwind/react'; export const dialog = style.animation({ to: 'opacity-0' }); const x = style(dialog('both'));",
            )
            .unwrap(),
        )),
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

#[test]
fn distinct_pools_may_publish_the_same_spelling() {
    let generators = Identifiers::with(Arc::new(FixedBodyGenerator));
    let mut session = ProjectSession::new(
        snapshot("red"),
        SessionMode::Watch,
        CompilerOptions::default(),
        generators,
    );
    let refs = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(Arc::new(
            SourceFile::new(
                FileId(1),
                "panel.tsx",
                "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.anchor);",
            )
            .unwrap(),
        )),
        None,
    );
    let update = session.update_module(refs);
    assert!(update.diagnostics.is_empty(), "{:?}", update.diagnostics);
    let artifact = &session.artifacts()["panel.tsx"];
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
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::with(Arc::new(FixedClassGenerator)),
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

/// Names every conditional helper with one fixed body, so two of them collide.
#[derive(Debug)]
struct FixedHelperGenerator;

impl IdentifierGenerator for FixedHelperGenerator {
    fn condition_helper(
        &self,
        _context: &ConditionContext,
        _hash: &IdentifierHash,
        _text_hash: &IdentifierHash,
    ) -> Result<GeneratedName<CustomPropertyName>, String> {
        Ok(GeneratedName::provided(
            CustomPropertyName::new("shared-if").expect("canonical test name"),
        ))
    }
}

#[test]
fn a_helper_name_is_checked_in_the_custom_property_pool() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::with(Arc::new(FixedHelperGenerator)),
    );
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "if.tsx",
            "import { style } from 'stillwind/react'; export const x = style('w-[--if(!--c,20px,10px)]', 'h-[--if(!--d,1px,2px)]');",
        )
        .unwrap(),
    );
    let update =
        session.update_module(OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source), None));
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

#[test]
fn css_a_producer_authors_is_reserved_when_the_host_supplies_it() {
    let mut session = ProjectSession::new(
        snapshot("red"),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::with(Arc::new(FixedBodyGenerator)),
    );
    let refs = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(Arc::new(
            SourceFile::new(
                FileId(1),
                "panel.tsx",
                "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.anchor);",
            )
            .unwrap(),
        )),
        None,
    );
    assert!(session.update_module(refs).diagnostics.is_empty());
    // The producer's own stylesheet names its anchor `shared`, which the generator also returned.
    let reported = session.reserve_authored_css("panel.css", ":root{anchor-name:shared;}");
    assert!(
        reported.values().flatten().any(|error| matches!(
            error.kind,
            CompileErrorKind::GeneratedIdentifierCollision { .. }
        )),
        "{reported:?}"
    );
    // Forgetting the stylesheet gives the name back.
    assert!(session.forget_authored_css("panel.css").is_empty());
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
    let theme = css_fixture::source(
        FileId(2),
        "theme",
        "@layer app-layer-0; @layer { @stillwind-slot utilities; }",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::with(Arc::new(FixedLayerGenerator)),
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

#[test]
fn a_generated_anchor_name_is_checked_against_the_authored_document() {
    let theme = css_fixture::source(
        FileId(2),
        "theme",
        ":root{anchor-name:shared;} @stillwind-slot utilities;",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
    let mut session = ProjectSession::new(
        snapshot,
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::with(Arc::new(FixedBodyGenerator)),
    );
    let refs = OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(Arc::new(
            SourceFile::new(
                FileId(1),
                "panel.tsx",
                "import { style } from 'stillwind/react'; export const menu = style.ref('menu'); const x = style(menu.anchor);",
            )
            .unwrap(),
        )),
        None,
    );
    let update = session.update_module(refs);
    // The generator names the anchor `shared`, which the authored declaration already owns.
    assert!(
        update.diagnostics.iter().any(|error| matches!(
            error.kind,
            CompileErrorKind::GeneratedIdentifierCollision { .. }
        )),
        "{:?}",
        update.diagnostics
    );
}
