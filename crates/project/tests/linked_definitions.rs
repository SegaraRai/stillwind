//! Definitions that linking takes from the declaring module.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::ToLeanString;

use support::snapshot;

use stillwind_compiler::{CompilerOptions, Identifiers, StylesheetPlacement};
use stillwind_diagnostics::CompileError;
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode, StylesheetCoverage, StylesheetSelection};
use stillwind_semantic::{
    AssetKind, ExportedValue, IdentityId, SymbolicValue, TypeFact, TypeFacts,
};
use stillwind_source::{FileId, Fingerprint, ModuleId, SourceFile, SourceRegistry};

const CONSUMER: &str = "import { style } from 'stillwind/react'; import { fade } from './frames'; export const x = style(fade('1s'));";

/// Returns the analysis of the module that declares `fade` with `frames` as its body.
fn producer(frames: &str) -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(
                SourceFile::new(
                    FileId(1),
                    None,
                    None,
                    format!(
                        "import {{ style }} from 'stillwind/react'; export const fade = style.keyframes({frames});"
                    ),
                )
                .unwrap(),
            ),
            ModuleId::from("frames.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    )
}

/// Returns the analysis of the module that uses `fade`, with a checker fact that
/// resolves the use to `identity`.
fn consumer(identity: &IdentityId) -> ModuleAnalysis {
    let source = SourceFile::new(FileId(3), None, None, CONSUMER).unwrap();
    let start = u32::try_from(CONSUMER.rfind("fade(").unwrap()).unwrap();
    let scan = OxcSourceAnalyzer.scan(
        Arc::new(source),
        ModuleId::from("app.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let facts = vec![TypeFact {
        start,
        end: start + 4,
        value: ExportedValue::Keyframes(SymbolicValue::exact(identity.clone())).into(),
    }];
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of(format_args!("{facts:?}").to_lean_string()),
        facts,
        assets: BTreeMap::new(),
    };
    OxcSourceAnalyzer.finalize(scan, Some(&facts))
}

fn keyframes_identity(analysis: &ModuleAnalysis) -> IdentityId {
    analysis
        .summary
        .assets
        .iter()
        .find(|asset| asset.identity.kind() == AssetKind::Keyframes)
        .unwrap()
        .identity
        .clone()
}

/// The consumer's artifact holds only its own rules and names the definition it
/// links, and a plan of the consumer alone adds the definition from the declaring
/// module. An edit of the frames changes the plan but not the consumer's artifact.
#[test]
fn plans_link_definitions_from_the_declaring_module() {
    let mut session = ProjectSession::new(
        snapshot(""),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    );
    let declaring = producer("{ from: 'opacity-0' }");
    let identity = keyframes_identity(&declaring);
    for analysis in [declaring, consumer(&identity)] {
        assert_eq!(
            session.update_module(analysis).diagnostics,
            [] as [CompileError; 0]
        );
    }
    let app = ModuleId::from("app.tsx");
    let artifact = Arc::clone(&session.artifacts()[&app]);
    assert_eq!(
        artifact.linked_definitions.iter().collect::<Vec<_>>(),
        [&identity]
    );
    assert!(!artifact.css.to_css().contains("@keyframes"));
    let plan = |session: &ProjectSession| {
        session
            .plan_stylesheet(
                StylesheetSelection {
                    modules: [app.clone()].into(),
                    coverage: StylesheetCoverage::Observed,
                },
                StylesheetPlacement::Carrier,
            )
            .unwrap()
    };
    let first = plan(&session);
    assert!(first.css.contains("@keyframes"), "{}", first.css);
    assert!(first.css.contains("opacity:calc(0 * 1%)"), "{}", first.css);
    assert!(first.producers.contains_key(&ModuleId::from("frames.tsx")));

    let edited = producer("{ from: 'opacity-50' }");
    assert_eq!(keyframes_identity(&edited), identity);
    assert_eq!(
        session.update_module(edited).diagnostics,
        [] as [CompileError; 0]
    );
    assert_eq!(session.artifacts()[&app].key(), artifact.key());
    let second = plan(&session);
    assert!(
        second.css.contains("opacity:calc(50 * 1%)"),
        "{}",
        second.css
    );
    assert!(
        !second.css.contains("opacity:calc(0 * 1%)"),
        "{}",
        second.css
    );
}
