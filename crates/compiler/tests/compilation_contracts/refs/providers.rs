//! Checks attachment requirements and conflicting provider properties.

use stillwind_diagnostics::LintDiagnostic;

use super::*;

#[test]
fn provider_attachment_checks_match_refs_within_the_owning_style_call() {
    for provider in ["asAnchor()", "asContainer()"] {
        for (body, expected) in [
            ("const x=style(r,r.PROVIDER);", 0),
            ("const x=style(r.PROVIDER);", 1),
            ("const x=style(other,r.PROVIDER);", 1),
            ("const marker=style(r);const x=style(r.PROVIDER);", 1),
            (
                "const View=()=> <div {...style(r)} {...style(r.PROVIDER)} />;",
                1,
            ),
            ("const View=()=> <div {...style(r,r.PROVIDER)} />;", 0),
            ("const base=style(r);const x=style(base,r.PROVIDER);", 0),
        ] {
            let body = body.replace("PROVIDER", provider);
            let (_, result) = compile(
                &format!("const r=style.ref();const other=style.ref();{body}"),
                "",
            );
            assert!(
                result.diagnostics.is_empty(),
                "{body}: {:?}",
                result.diagnostics
            );
            let findings = result
                .lint_findings
                .iter()
                .filter(|finding| {
                    matches!(
                        finding.diagnostic,
                        LintDiagnostic::MissingRefAttachment { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                findings.len(),
                expected,
                "{body}: {:?}",
                result.lint_findings
            );
        }
    }
}

#[test]
fn provider_property_conflicts_include_overlapping_declarations() {
    let (_, conflict) = compile(
        "const r=style.ref();const x=style(r,r.asContainer('size'),'[container-type:normal]');",
        "",
    );
    assert!(
        conflict
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::ProviderConflict { .. })),
        "{:?}",
        conflict.lint_findings
    );
    let (_, omitted) = compile(
        "const r=style.ref();const x=style(r,r.asContainer(),'[container-type:normal]');",
        "",
    );
    assert!(
        !omitted
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::ProviderConflict { .. }))
    );
    let (_, duplicate) = compile(
        "const r=style.ref();const x=style(r,r.asContainer('size'),r.asContainer('normal'));",
        "",
    );
    assert!(
        duplicate
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::ProviderConflict { .. }))
    );
}

#[test]
fn exclusive_provider_marker_gates_do_not_conflict() {
    let (_, result) = compile(
        "const a=style.ref();const b=style.ref();const x=style(flag?a:b,a.asContainer('size'),b.asContainer('normal'));",
        "",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        !result.lint_findings.iter().any(|f| matches!(
            f.diagnostic,
            LintDiagnostic::ProviderConflict { .. } | LintDiagnostic::MissingRefAttachment { .. }
        )),
        "{:?}",
        result.lint_findings
    );
}
