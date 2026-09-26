//! The one composition-order lint case that needs cross-module checker facts.
//!
//! The rule itself and the same-module session wiring are covered by the unit tests in
//! `composition_lint`; a frontend-only analysis cannot supply facts for a base that
//! lives in another module, so this stays an integration target.

#[path = "../../test-support/css.rs"]
mod css_fixture;

use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, Identifiers};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_semantic::ModuleId;
use stillwind_source::{FileId, SourceFile, SourceRegistry};

fn snapshot() -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(FileId(2), "@theme { --color-brand: red; }").unwrap(),
        "test.css",
        DesignSystemOptions::default(),
    )
    .unwrap()
}

fn analysis(name: &str, source: &str) -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            Arc::new(SourceFile::new(FileId(1), None, None, source).unwrap()),
            ModuleId::from(name),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        ),
        None,
    )
}

fn session() -> ProjectSession {
    ProjectSession::new(
        snapshot(),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::default(),
    )
}

/// Cross-module composition needs checker facts, which this frontend-only test cannot
/// supply.
#[test]
fn imported_base_composition_requires_checker_facts() {
    let mut session = session();
    session.update_module(analysis(
        "base.tsx",
        r"import { style } from 'stillwind/react';
export const base = style('p-10');
",
    ));
    let update = session.update_module(analysis(
        "app.tsx",
        r"import { style } from 'stillwind/react';
import { base } from './base';
export const derived = style(base, 'p-4');
",
    ));
    assert!(
        update
            .diagnostics
            .iter()
            .any(|error| error.kind == CompileErrorKind::RefFactsRequired),
        "expected a facts requirement, got {:?}",
        update.diagnostics
    );
}

/// A logical and a physical member of one property group interfere. The synthetic
/// artifacts above cannot express this because their bodies encode one property each.
#[test]
fn a_logical_base_and_a_physical_base_are_reported() {
    let mut session = session();
    session.update_module(analysis(
        "app.tsx",
        r"import { style } from 'stillwind/react';
const a = style('mt-4');
const b = style('ms-4');
const c = style(a, b, 'text-brand');
",
    ));
    let artifact = session.artifacts().get("app.tsx").expect("published");
    let findings = artifact
        .lint_findings
        .iter()
        .filter(|finding| finding.code() == "ambiguous-base-order")
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "{findings:?}");
    let message = findings[0].kind.to_string();
    assert!(message.contains("margin"), "{message}");
}

/// A `swlint-*` directive on the derived call removes the finding.
#[test]
fn suppression_directives_remove_the_findings() {
    let mut session = session();
    session.update_module(analysis(
        "app.tsx",
        r"import { style } from 'stillwind/react';
const a = style('p-10');
const b = style('p-4');
// swlint-disable-next-line ambiguous-base-order
const c = style(a, b, 'text-brand');
",
    ));
    let artifact = session.artifacts().get("app.tsx").expect("published");
    assert!(
        !artifact
            .lint_findings
            .iter()
            .any(|finding| finding.code() == "ambiguous-base-order"),
        "suppression did not apply"
    );
}
