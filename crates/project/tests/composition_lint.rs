//! Composition ordering and suppression through a project session.

mod support;

use support::{analysis, brand_snapshot};

use stillwind_compiler::{CompilerOptions, Identifiers};
use stillwind_diagnostics::{
    CanonicalUtilitiesOverride, CompileDiagnostic, LintDiagnostic, LintOverrides, LintSeverity,
};
use stillwind_project::{ProjectSession, SessionMode};
use stillwind_source::{ModuleId, SourceRegistry};

fn session() -> ProjectSession {
    ProjectSession::new(
        brand_snapshot("red"),
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
            .any(|error| error.diagnostic == CompileDiagnostic::RefFactsRequired),
        "expected a facts requirement, got {:?}",
        update.diagnostics
    );
}

/// A logical and a physical member of one property group interfere.
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
    let artifact = session
        .artifacts()
        .get(&ModuleId::from("app.tsx"))
        .expect("published");
    let findings = artifact
        .lint_findings
        .iter()
        .filter(|finding| finding.code() == "ambiguous-base-order")
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "{findings:?}");
    let message = findings[0].diagnostic.to_string();
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
    let artifact = session
        .artifacts()
        .get(&ModuleId::from("app.tsx"))
        .expect("published");
    assert!(
        !artifact
            .lint_findings
            .iter()
            .any(|finding| finding.code() == "ambiguous-base-order"),
        "suppression did not apply"
    );
}

#[test]
fn suppression_usage_is_recomputed_with_source_and_composition_findings() {
    let mut session = ProjectSession::new(
        brand_snapshot("red"),
        &SourceRegistry::default(),
        SessionMode::Watch,
        CompilerOptions {
            lint: LintOverrides {
                canonical_utilities: Some(CanonicalUtilitiesOverride {
                    severity: Some(LintSeverity::Off),
                    ..Default::default()
                }),
                no_important: Some(LintSeverity::Warn),
                no_unused_lint_suppressions: Some(LintSeverity::Warn),
                ..LintOverrides::default()
            },
            ..CompilerOptions::default()
        },
        Identifiers::default(),
    );
    let source = r"import { style } from 'stillwind/react';
const a=style('p-10'); const b=style('p-4');
// swlint-disable-next-line ambiguous-base-order
const c=style(a,b,'text-brand');
// swlint-disable-next-line no-important
const d=style('[display:flex]!');
";
    session.update_module(analysis("app.tsx", source));
    let module = ModuleId::from("app.tsx");
    assert!(
        session.artifacts()[&module].lint_findings.is_empty(),
        "{:?}",
        session.artifacts()[&module].lint_findings
    );
    session.update_module(analysis(
        "app.tsx",
        &source.replace(
            "const d=style('[display:flex]!')",
            "const d=style('[display:flex]')",
        ),
    ));
    let findings = &session.artifacts()[&module].lint_findings;
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(
        matches!(&findings[0].diagnostic, LintDiagnostic::UnusedLintSuppression { rule } if rule == "no-important")
    );
}
