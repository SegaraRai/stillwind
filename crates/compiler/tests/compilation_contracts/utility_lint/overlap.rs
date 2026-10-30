//! Checks competing contributions, duplicates, and suppression usage.

use stillwind_diagnostics::LintDiagnostic;

use super::*;

#[test]
fn utility_pairs_respect_css_contexts_source_choices_and_shorthand_policy() {
    for (code, expected) in [
        ("const x=style('p2 p2');", Some(LintKind::DuplicateUtility)),
        (
            "const x=style('p2', ['p4']);",
            Some(LintKind::ConflictingUtilities),
        ),
        (
            "const x=style('p2 px4');",
            Some(LintKind::ConflictingUtilities),
        ),
        ("const x=style('px4 py4');", None),
        (
            "const x=style('pl4 ps2');",
            Some(LintKind::ConflictingUtilities),
        ),
        ("const x=style('p2 hover:p4');", None),
        ("const x=style(flag?'p2':'p4');", None),
        (
            "const x=style('hover:p2',{'hover:':'p2'});",
            Some(LintKind::DuplicateUtility),
        ),
    ] {
        assert_eq!(
            kinds(code, CSS, &CompilerOptions::default()),
            expected.into_iter().collect::<Vec<_>>(),
            "{code}"
        );
    }
    let options = CompilerOptions {
        lint: LintOverrides {
            conflicting_utilities: Some(ConflictingUtilitiesOverride {
                allow_shorthand_overrides: Some(true),
                ..ConflictingUtilitiesOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    assert_eq!(
        kinds("const x=style('p2 px4');", CSS, &options),
        [] as [LintKind; 0]
    );
    assert_eq!(
        kinds("const x=style('px4 ps2');", CSS, &options),
        [] as [LintKind; 0]
    );
    assert_eq!(
        kinds("const x=style('p2 p4');", CSS, &options),
        [LintKind::ConflictingUtilities]
    );
    assert_eq!(
        kinds("const x=style('pl4 ps2');", CSS, &options),
        [LintKind::ConflictingUtilities]
    );
}

#[test]
fn shared_composition_declarations_and_one_candidate_expansion_do_not_compete() {
    let theme = "@utility x {--x:1px;translate:var(--x) var(--y)} @utility y {--y:2px;translate:var(--x) var(--y)} @utility many {color:red; &:hover {color:blue;} }";
    assert_eq!(
        kinds(
            "const x=style('x y many');",
            theme,
            &CompilerOptions::default()
        ),
        [] as [LintKind; 0]
    );
    let (_, result) = compile("const x=style('many many');", theme);
    assert_eq!(
        result
            .lint_findings
            .iter()
            .map(LintFinding::kind)
            .collect::<Vec<_>>(),
        [LintKind::DuplicateUtility]
    );
}

#[test]
fn duplicate_locations_map_escaped_source_and_suppression() {
    let (source, result) = compile(r"const x=style('p2 \u00702');", CSS);
    let finding = result
        .lint_findings
        .iter()
        .find(|f| matches!(f.diagnostic, LintDiagnostic::DuplicateUtility { .. }))
        .unwrap();
    assert_eq!(
        &source.text()[finding.info.labels[0].span.start.0 as usize
            ..finding.info.labels[0].span.end.0 as usize],
        r"\u00702"
    );
    assert_eq!(
        kinds(
            "\n// swlint-disable-next-line duplicate-utility\nconst x=style('p2 p2');",
            CSS,
            &CompilerOptions::default()
        ),
        [] as [LintKind; 0]
    );
}

#[test]
fn suppression_usage_includes_findings_detected_after_source_analysis() {
    let options = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        unused_lint_suppression: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    let used = "\n// swlint-disable-next-line no-arbitrary-colors\nconst x=style('paint-[#abc]');";
    assert_eq!(kinds(used, VALUE_CSS, &options), [] as [LintKind; 0]);
    let unused =
        "\n// swlint-disable-next-line no-arbitrary-colors\nconst x=style('paint-primary');";
    assert_eq!(
        kinds(unused, VALUE_CSS, &options),
        [LintKind::UnusedLintSuppression]
    );
    let invalid = "\n// swlint-disable-next-line invalid-rule\nconst x=style('paint-primary');";
    assert_eq!(
        kinds(invalid, VALUE_CSS, &options),
        [LintKind::InvalidLintSuppression]
    );
    let off = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Off),
        unused_lint_suppression: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    assert_eq!(kinds(unused, VALUE_CSS, &off), [] as [LintKind; 0]);
}

#[test]
fn motion_definitions_apply_source_policies() {
    let css = format!("@custom-variant calm (@media print); {VALUE_CSS}");
    let options = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    for code in [
        "export const a=style.keyframes({from:'paint-[#abc]'});",
        "export const a=style.viewTransition({old:'paint-[#abc]'});",
        "export const a=style.viewTransition({old:'calm:paint-[#abc]'});",
    ] {
        assert_eq!(
            kinds(code, &css, &options),
            [LintKind::NoArbitraryColors],
            "{code}"
        );
    }
}
