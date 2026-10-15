//! Composition-aware checks through the public compiler API.

use stillwind_diagnostics::{LintFinding, LintKind};

use super::compile;

const CSS: &str =
    "@utility pad { padding:1px; } @utility pad2 { padding:2px; } @utility color { color:red; }";

fn findings(code: &str) -> Vec<LintKind> {
    let (_, result) = compile(code, CSS);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.lint_findings.iter().map(LintFinding::kind).collect()
}

#[test]
fn inherited_properties_and_conditional_classes_preserve_interference() {
    for code in [
        "const a=style('pad',flag&&'color'); const b=style('pad2'); const x=style(a,b);",
        "const a=style('pad'); const middle=style(a); const b=style('pad2'); const x=style(middle,b);",
    ] {
        assert_eq!(
            findings(code),
            vec![LintKind::AmbiguousStyleBaseOrder],
            "{code}"
        );
    }
}

#[test]
fn alternative_bases_and_repeated_inheritance_do_not_compete() {
    for code in [
        "const a=style('pad'); const b=style('pad2'); const x=style(flag?a:b);",
        "const a=style('pad'); const b=style(a); const x=style(a,b);",
    ] {
        assert!(findings(code).is_empty(), "{code}");
    }
}

#[test]
fn attachments_are_resolved_from_bases_instead_of_assumed() {
    assert_eq!(
        findings(
            "const r=style.ref(); const a=style(r); const b=style(a); const x=style(b,r.asContainer('size'));"
        ),
        [] as [LintKind; 0]
    );
    assert_eq!(
        findings(
            "const r=style.ref(); const a=style('color'); const x=style(a,r.asContainer('size'));"
        ),
        vec![LintKind::MissingRefAttachment]
    );
    assert_eq!(
        findings("const r=style.ref(); const x=style(flag?r:r.asContainer('size'));"),
        vec![LintKind::MissingRefAttachment]
    );
}

#[test]
fn inherited_provider_writes_conflict_with_direct_properties() {
    assert_eq!(
        findings(
            "const r=style.ref(); const a=style(r,r.asContainer('size')); const b=style(a); const x=style(b,'[container-type:normal]');"
        ),
        vec![LintKind::ConflictingProviderProperties]
    );
}

#[test]
fn transition_names_follow_the_target_and_transitive_bases() {
    assert_eq!(
        findings(
            "const vt=style.viewTransition({old:'color'}); const a=style(vt.for('a')); const b=style(a); const x=style(b);"
        ),
        [] as [LintKind; 0]
    );
    assert_eq!(
        findings(
            "const vt=style.viewTransition({old:'color'}); const x=style(vt,{'[&>span]:':'[view-transition-name:hero]'});"
        ),
        vec![LintKind::MissingViewTransitionName]
    );
    assert_eq!(
        findings(
            "const vt=style.viewTransition({old:'color'}); const a=style(vt.for('a')); const b=style(a); const x=style(a,b);"
        ),
        [] as [LintKind; 0]
    );
    assert_eq!(
        findings(
            "const vt=style.viewTransition({old:'color'}); const a=style(vt.for('a')); const x=style(a,vt.for('b'));"
        ),
        vec![LintKind::DuplicateViewTransitionName]
    );
}

#[test]
fn transition_class_replacement_sees_transitive_inheritance() {
    let kinds = findings(
        "const a=style.viewTransition({old:'color'}); const b=style.viewTransition({old:'color'}); const base=style(a.for('a')); const middle=style(base); const x=style(middle,b);",
    );
    assert_eq!(kinds, vec![LintKind::SplitViewTransitionClass]);
    assert_eq!(
        findings(
            "const a=style.viewTransition({old:'color'}); const base=style(a.for('a')); const x=style(base,flag&&a);"
        ),
        vec![LintKind::SplitViewTransitionClass]
    );
    assert_eq!(
        findings(
            "const a=style.viewTransition({old:'color'}); const base=style(a.for('a')); const x=style(base,flag?a:a);"
        ),
        [] as [LintKind; 0]
    );
}

#[test]
fn expanded_selector_and_at_rule_contexts_control_base_interference() {
    let css = format!(
        "{CSS} @custom-variant hover (&:hover); @custom-variant active (&:active); @custom-variant wide (@media (width > 40rem)); @custom-variant prefix (.enabled &);"
    );
    for (a, b, conflict) in [
        ("pad", "hover:pad2", false),
        ("hover:pad", "active:pad2", false),
        ("hover:pad", "hover:pad2", true),
        ("wide:pad", "pad2", false),
        ("wide:pad", "wide:pad2", true),
        ("prefix:pad", "pad2", false),
        ("prefix:pad", "prefix:pad2", true),
    ] {
        let (_, result) = compile(
            &format!("const a=style('{a}'); const b=style('{b}'); const x=style(a,b);"),
            &css,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result
                .lint_findings
                .iter()
                .any(|f| f.kind() == LintKind::AmbiguousStyleBaseOrder),
            conflict,
            "{a}, {b}: {:?}",
            result.lint_findings
        );
    }
}

#[test]
fn independent_name_uses_are_counted_and_choices_stay_exclusive() {
    assert_eq!(
        findings(
            "const vt=style.viewTransition({old:'color'}); const x=style(vt,'[view-transition-name:a] [view-transition-name:b]');"
        ),
        vec![
            LintKind::DuplicateViewTransitionName,
            LintKind::ConflictingUtilities
        ]
    );
    assert_eq!(
        findings(
            "const vt=style.viewTransition({old:'color'}); const a=style(vt.for('a')); const b=style(vt.for('b')); const x=style(flag?a:b);"
        ),
        [] as [LintKind; 0]
    );
}
