//! Composition-order lints through a real session.
//!
//! These drive the public session API with analyses from the Oxc frontend, so they cover
//! the whole path a host sees: the overlap is read from published rules, the finding is
//! attached to the derived module's artifact, and `swlint-*` directives suppress it.

use std::sync::Arc;

use stillwind_compiler::{CompilerOptions, Identifiers};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{ModuleAnalysis, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_source::{FileId, SourceFile};

#[path = "../../test-support/css.rs"]
mod css_fixture;

fn snapshot() -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &crate::css_fixture::source(FileId(2), "theme", "@theme { --color-brand: red; }").unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap()
}

fn analysis(name: &str, source: &str) -> ModuleAnalysis {
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(Arc::new(SourceFile::new(FileId(1), name, source).unwrap())),
        None,
    )
}

fn session() -> ProjectSession {
    ProjectSession::new(
        snapshot(),
        SessionMode::Watch,
        CompilerOptions::default(),
        Identifiers::builtin(),
    )
}

#[test]
fn independent_bases_that_share_a_property_are_reported() {
    let mut session = session();
    session.update_module(analysis(
        "app.tsx",
        r"import { style } from 'stillwind/react';
const a = style('p-10');
const b = style('p-4');
const c = style(a, b, 'text-brand');
",
    ));
    let artifact = session.artifacts().get("app.tsx").expect("published");
    assert!(
        artifact
            .lint_findings
            .iter()
            .any(|finding| finding.code() == "ambiguous-base-order"),
        "expected ambiguous-base-order, got {:?}",
        artifact
            .lint_findings
            .iter()
            .map(stillwind_diagnostics::LintFinding::code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn independent_bases_with_disjoint_properties_stay_silent() {
    let mut session = session();
    session.update_module(analysis(
        "app.tsx",
        r"import { style } from 'stillwind/react';
const a = style('p-10');
const b = style('text-brand');
const c = style(a, b, 'm-2');
",
    ));
    let artifact = session.artifacts().get("app.tsx").expect("published");
    assert!(
        !artifact
            .lint_findings
            .iter()
            .any(|finding| finding.code() == "ambiguous-base-order"),
        "unexpected finding"
    );
}

#[test]
fn a_shorthand_base_and_a_longhand_base_are_reported() {
    let mut session = session();
    session.update_module(analysis(
        "app.tsx",
        r"import { style } from 'stillwind/react';
const a = style('p-10');
const b = style('pt-4');
const c = style(a, b, 'text-brand');
",
    ));
    let artifact = session.artifacts().get("app.tsx").expect("published");
    assert!(
        artifact
            .lint_findings
            .iter()
            .any(|finding| finding.code() == "ambiguous-base-order"),
        "expected ambiguous-base-order, got {:?}",
        artifact
            .lint_findings
            .iter()
            .map(stillwind_diagnostics::LintFinding::code)
            .collect::<Vec<_>>()
    );
}

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

/// Cross-module composition needs checker facts, which this frontend-only test cannot
/// supply. The rule itself is covered by the unit tests in `composition_lint`, and the
/// session wiring by the same-module cases here.
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
