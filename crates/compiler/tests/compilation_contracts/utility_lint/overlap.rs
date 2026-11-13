//! Checks competing contributions, duplicates, and suppression usage.

use lean_string::LeanStr;

use stillwind_css::WritingMode;
use stillwind_diagnostics::LintDiagnostic;

use super::*;

/// Returns compiler options that check logical and physical utilities in `modes`.
fn writing_modes(modes: &[WritingMode]) -> CompilerOptions {
    CompilerOptions {
        lint: LintOverrides {
            conflicting_utilities: Some(ConflictingUtilitiesOverride {
                writing_modes: Some(modes.to_vec()),
                ..ConflictingUtilitiesOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    }
}

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
fn pairs_with_an_ignored_utility_do_not_compete() {
    let options = CompilerOptions {
        lint: LintOverrides {
            conflicting_utilities: Some(ConflictingUtilitiesOverride {
                ignored_utilities: Some(vec!["p?".into()]),
                ..ConflictingUtilitiesOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    for (code, expected) in [
        ("const x=style('p2 px4');", None),
        ("const x=style('hover:p2! hover:px4');", None),
        ("const x=style('p2', ['px4']);", None),
        (
            "const x=style('p2 px4 px4');",
            Some(LintKind::DuplicateUtility),
        ),
        (
            "const x=style('px4 pl4');",
            Some(LintKind::ConflictingUtilities),
        ),
    ] {
        assert_eq!(
            kinds(code, CSS, &options),
            expected.into_iter().collect::<Vec<_>>(),
            "{code}"
        );
    }
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
fn a_declaration_that_copies_the_written_custom_property_does_not_compete() {
    let theme = concat!(
        "@utility bt {border-top-style:var(--bs);border-top-width:1px} ",
        "@utility dashed {--bs:dashed;border-style:dashed} ",
        "@utility dotted {--bs:dotted;border-style:dotted} ",
        "@utility other {--other:dashed;border-style:dashed} ",
        "@utility dy {& > * {border-block-style:var(--bs)}} ",
        "@utility d-dashed {& > * {--bs:dashed;border-style:dashed}} ",
        "@utility text-sm {font-size:0.875rem;line-height:var(--leading, 1.4)} ",
        "@utility leading-relaxed {--leading:1.6;line-height:1.6}",
    );
    for (code, expected) in [
        ("const x=style('bt dashed');", None),
        ("const x=style('dashed bt');", None),
        ("const x=style('dy d-dashed');", None),
        ("const x=style('text-sm leading-relaxed');", None),
        (
            "const x=style('bt other');",
            Some(LintKind::ConflictingUtilities),
        ),
        (
            "const x=style('dashed dotted');",
            Some(LintKind::ConflictingUtilities),
        ),
    ] {
        assert_eq!(
            kinds(code, theme, &CompilerOptions::default()),
            expected.into_iter().collect::<Vec<_>>(),
            "{code}"
        );
    }
}

#[test]
fn logical_and_physical_utilities_compete_in_the_configured_writing_modes() {
    let theme = concat!(
        "@utility px4 {padding-inline:1rem} @utility pt2 {padding-top:0.5rem} ",
        "@utility pl2 {padding-left:0.5rem} @utility pbs2 {padding-block-start:0.5rem} ",
        "@utility pb2 {padding-bottom:0.5rem} @utility p2 {padding:0.5rem}",
    );
    let vertical = writing_modes(&[WritingMode::VerticalRl]);
    let every = writing_modes(&WritingMode::ALL);
    let none = writing_modes(&[]);
    // Each case lists whether the pair conflicts by default, in `vertical-rl`, in every
    // writing mode, and with no writing mode.
    for (code, expected) in [
        ("const x=style('px4 pt2');", [false, true, true, false]),
        ("const x=style('px4 pl2');", [true, false, true, false]),
        ("const x=style('pbs2 pb2');", [false, false, false, false]),
        ("const x=style('p2 px4');", [true, true, true, true]),
    ] {
        let actual = [&CompilerOptions::default(), &vertical, &every, &none]
            .map(|options| kinds(code, theme, options) == [LintKind::ConflictingUtilities]);
        assert_eq!(actual, expected, "{code}");
    }
}

#[test]
fn conflict_findings_name_the_logical_group_of_a_writing_mode_overlap() {
    let (_, result) = compile_with_options(
        "const x=style('px4 pt2');",
        "@utility px4 {padding-inline:1rem} @utility pt2 {padding-top:0.5rem}",
        &writing_modes(&[WritingMode::VerticalRl]),
    );
    let notes: Vec<_> = result
        .lint_findings
        .iter()
        .flat_map(|finding| finding.info.notes.iter().map(LeanStr::as_str))
        .collect();
    assert_eq!(
        notes,
        [
            "`padding-inline` and `padding-top` overlap through the `padding` logical group, so the writing mode decides which sides they share; set both with logical properties or both with physical properties"
        ]
    );
}

#[test]
fn conflict_findings_offer_a_named_logical_utility_from_the_design_system() {
    let fixture = CompilationFixture::new(&with_defaults(""), &writing_modes(&WritingMode::ALL));
    for (code, expected) in [
        (
            "const x=style('px-2 pt-2');",
            [concat!(
                "`padding-inline` and `padding-top` overlap through the `padding` logical group, ",
                "so the writing mode decides which sides they share; set both with logical ",
                "properties or both with physical properties; to use logical properties, replace ",
                "`pt-2` with `pbs-2`, which sets `padding-block-start` in place of `padding-top` ",
                "in horizontal left-to-right writing modes",
            )],
        ),
        (
            "const x=style('md:mr-[3px] md:mx-4');",
            [concat!(
                "`margin-right` and `margin-inline` overlap through the `margin` logical group, ",
                "so the writing mode decides which sides they share; set both with logical ",
                "properties or both with physical properties; to use logical properties, replace ",
                "`md:mr-[3px]` with `md:me-0.75`, which sets `margin-inline-end` in place of ",
                "`margin-right` in horizontal left-to-right writing modes",
            )],
        ),
    ] {
        let (_, result) = fixture.compile(code);
        let notes: Vec<_> = result
            .lint_findings
            .iter()
            .filter(|finding| finding.kind() == LintKind::ConflictingUtilities)
            .flat_map(|finding| finding.info.notes.iter().map(LeanStr::as_str))
            .collect();
        assert_eq!(notes, expected, "{code}");
    }
    let (_, result) = CompilationFixture::new(
        &with_defaults(""),
        &writing_modes(&[WritingMode::VerticalRl]),
    )
    .compile("const x=style('px-2 pt-2');");
    let notes: Vec<_> = result
        .lint_findings
        .iter()
        .flat_map(|finding| finding.info.notes.iter().map(LeanStr::as_str))
        .collect();
    assert_eq!(
        notes,
        [concat!(
            "`padding-inline` and `padding-top` overlap through the `padding` logical group, ",
            "so the writing mode decides which sides they share; set both with logical ",
            "properties or both with physical properties",
        )],
        "the replacement uses the sides of horizontal-tb"
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
