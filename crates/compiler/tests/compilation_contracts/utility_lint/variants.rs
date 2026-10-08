//! Checks breakpoint and direct styling policies under variants.

use super::*;

const BREAKPOINT_CSS: &str =
    "@theme { --breakpoint-md:48rem; } @utility flex {display:flex} @utility block {display:block}
@variant tablet { @media (width >= 900px) { @slot; } }
@variant screen-* { @media (width >= --value([length])) { @slot; } }
@variant viewport-* { @media (width >= --value()) { @slot; } }
@variant responsive-* { @media (width >= --value([length])) { &:hover { @slot; } } }
@variant mixed-* { @media (width >= 900px) and (color: --value([integer])) { @slot; } }
@utility internal { @media (width >= 900px) {display:flex;} }";

#[test]
fn arbitrary_breakpoints_distinguish_value_sources_and_exclude_containers() {
    let theme = with_defaults(BREAKPOINT_CSS);
    let options = CompilerOptions {
        lint: LintOverrides {
            no_arbitrary_breakpoints: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    let fixture = CompilationFixture::new(&theme, &options);
    for variant in [
        "max-[900px]",
        "min-[48rem]",
        "screen-[calc(40rem_+_1px)]",
        "viewport-[900px]",
        "group-responsive-[900px]",
        "max-[900px]/ignored",
        "[@media(width>=900px)]",
        "[@media(900px<width)]",
        "[@media(min-height:600px)]",
        "[@media_print_and_(width>=900px)]",
    ] {
        assert_eq!(
            kinds_in(&format!("const x=style('{variant}:flex');"), &fixture),
            [LintKind::NoArbitraryBreakpoints],
            "{variant}"
        );
    }
    for variant in [
        "md",
        "min-md",
        "max-md",
        "tablet",
        "mixed-[8]",
        "@min-[900px]",
        "@max-[900px]",
        "@[900px]",
        "[@container(width>=900px)]",
        "[@media(orientation:portrait)]",
        "supports-[width:900px]",
    ] {
        assert_eq!(
            kinds_in(&format!("const x=style('{variant}:flex');"), &fixture),
            [] as [LintKind; 0],
            "{variant}"
        );
    }
    assert_eq!(
        kinds_in("const x=style('internal');", &fixture),
        [] as [LintKind; 0]
    );
    assert_eq!(
        kinds(
            "const x=style('max-[900px]:flex');",
            &format!("{theme} @variant max-* {{ &:hover {{@slot;}} }}"),
            &options
        ),
        [] as [LintKind; 0]
    );
    assert_eq!(
        kinds(
            "const x=style('max-[900px]:flex');",
            &theme,
            &CompilerOptions::default()
        ),
        [] as [LintKind; 0]
    );
    let off = CompilerOptions {
        lint: LintOverrides {
            no_arbitrary_breakpoints: Some(LintSeverity::Off),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    assert_eq!(
        kinds("const x=style('max-[900px]:flex');", &theme, &off),
        [] as [LintKind; 0]
    );
}

#[test]
fn breakpoint_findings_point_to_each_variant_and_report_wrappers_once() {
    let theme = with_defaults(BREAKPOINT_CSS);
    let options = CompilerOptions {
        lint: LintOverrides {
            no_arbitrary_breakpoints: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    let fixture = CompilationFixture::new(&theme, &options);
    for (code, expected) in [
        (
            r"const x=style('max-[900px]:min-[48rem]:flex');",
            vec!["max-[900px]", "min-[48rem]"],
        ),
        (
            r"const x=style('max-[900\u0070x]:flex');",
            vec![r"max-[900\u0070x]"],
        ),
        (
            r"const x=style({'max-[900px]:':['flex','block']});",
            vec!["max-[900px]"],
        ),
    ] {
        let (source, result) = fixture.compile(code);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let spans = result
            .lint_findings
            .iter()
            .filter(|finding| finding.kind() == LintKind::NoArbitraryBreakpoints)
            .map(|finding| {
                &source.text()[finding.info.labels[0].span.start.0 as usize
                    ..finding.info.labels[0].span.end.0 as usize]
            })
            .collect::<Vec<_>>();
        assert_eq!(spans, expected, "{code}");
    }
    let (_, result) = fixture.compile(
        "// swlint-disable-next-line no-arbitrary-breakpoints\nconst x=style('max-[900px]:flex');",
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
}

#[test]
fn direct_styling_policy_allows_pseudo_elements_and_utility_internal_selectors() {
    assert_eq!(
        kinds(
            "const x=style('child:p2');",
            CSS,
            &CompilerOptions::default()
        ),
        [] as [LintKind; 0]
    );
    let theme = format!(
        "{CSS} @custom-variant parent (.parent &); @custom-variant has (&:has(>img)); @custom-variant before (&::before); @custom-variant legacy (&:after); @custom-variant part (&::part(label)); @custom-variant pseudos (&::before, &::after); @custom-variant child-before (&>span::before); @custom-variant mixed (&::before, &>span::after); @custom-variant list (:is(&,span)::before);"
    );
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_direct_styling: Some(LintSeverity::Warn),
            ..Default::default()
        },
        ..Default::default()
    };
    for utility in [
        "child",
        "hover:child",
        "parent:p2",
        "has:p2",
        "before:p2",
        "legacy:p2",
        "part:p2",
        "pseudos:p2",
        "hover:before:p2",
    ] {
        assert!(
            kinds(&format!("const x=style('{utility}');"), &theme, &options).is_empty(),
            "{utility}"
        );
    }
    for utility in [
        "child:p2",
        "child-before:p2",
        "mixed:p2",
        "list:p2",
        "child:before:p2",
    ] {
        assert_eq!(
            kinds(&format!("const x=style('{utility}');"), &theme, &options),
            [LintKind::PreferDirectStyling],
            "{utility}"
        );
    }
    assert_eq!(
        kinds(
            "// swlint-disable-next-line prefer-direct-styling\nconst x=style('child-before:p2');",
            &theme,
            &options,
        ),
        [] as [LintKind; 0]
    );
}
