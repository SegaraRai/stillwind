//! Source utility policies exercised through the public compiler API.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{
    ArbitrarySpacingOverride, CanonicalUtilitiesOverride, ConflictingUtilitiesOverride,
    LengthUnitRule, LengthUnitsOverride, LintDiagnostic, LintFinding, LintKind, LintOverrides,
    LintSeverity, RestrictedDeclarationRule, RestrictedDeclarationsOverride,
    RestrictedThemeTokensOverride, RestrictedUtilitiesOverride, RestrictedUtilityRule,
};
use stillwind_test_support::css;

use super::compile;
use super::support::compile_with_options;

const CSS: &str = "@utility p2 {padding:0.5rem} @utility p4 {padding:1rem} @utility px4 {padding-inline:1rem} @utility ps2 {padding-inline-start:0.5rem} @utility pl4 {padding-left:1rem} @utility py4 {padding-block:1rem} @custom-variant hover (&:hover); @custom-variant child (&>span); @utility child { &>span { color:red; } }";

fn kinds(code: &str, theme: &str, options: &CompilerOptions) -> Vec<LintKind> {
    let (_, result) = compile_with_options(code, theme, options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.lint_findings.iter().map(LintFinding::kind).collect()
}

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
    let theme = css::with_defaults(BREAKPOINT_CSS);
    let options = CompilerOptions {
        lint: LintOverrides {
            no_arbitrary_breakpoints: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
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
            kinds(
                &format!("const x=style('{variant}:flex');"),
                &theme,
                &options
            ),
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
            kinds(
                &format!("const x=style('{variant}:flex');"),
                &theme,
                &options
            ),
            [] as [LintKind; 0],
            "{variant}"
        );
    }
    assert_eq!(
        kinds("const x=style('internal');", &theme, &options),
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
        kinds(
            "const x=style('max-[900px]:flex');",
            &format!("{theme} @stillwind {{no-arbitrary-breakpoints:deny;}}"),
            &off
        ),
        [] as [LintKind; 0]
    );
}

#[test]
fn breakpoint_findings_point_to_each_variant_and_report_wrappers_once() {
    let theme = css::with_defaults(BREAKPOINT_CSS);
    let options = CompilerOptions {
        lint: LintOverrides {
            no_arbitrary_breakpoints: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
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
        let (source, result) = compile_with_options(code, &theme, &options);
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
    let (_, result) = compile_with_options(
        "// swlint-disable-next-line no-arbitrary-breakpoints\nconst x=style('max-[900px]:flex');",
        &theme,
        &options,
    );
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
}

#[test]
fn restricted_theme_tokens_cover_non_color_values_and_variant_selections() {
    let theme = "@theme inline {--spacing-ref-small:1rem; --spacing-gutter:var(--spacing-ref-small); --duration-ref-fast:100ms; --duration-enter:var(--duration-ref-fast); --breakpoint-md:48rem; }
@utility p-* {padding:--value(--spacing-*,[length]);} @utility duration-* {transition-duration:--value(--duration-*,[time]);} @utility flex {display:flex;}
@variant speed-* { & {transition-duration:--value(--duration-*); @slot;} }
@variant screen-* { @media (width>=--value([length])) {@slot;} }
@variant choose-* { @media (width>=--value(--spacing-*,[length])) { @supports (display:--value([color])) {@slot;} } }
@variant choose-* { @media (width>=--value(--breakpoint-*,[length])) {@slot;} }";
    let theme = css::with_defaults(theme);
    let options = CompilerOptions {
        lint: LintOverrides {
            no_restricted_theme_tokens: Some(RestrictedThemeTokensOverride {
                tokens: Some(vec![
                    "--spacing-ref-*".into(),
                    "--duration-ref-*".into(),
                    "--breakpoint-*".into(),
                ]),
                ..RestrictedThemeTokensOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    for utility in [
        "p-ref-small",
        "duration-ref-fast",
        "[padding:var(--spacing-ref-small)]",
        "speed-ref-fast:flex",
        "md:flex",
        "choose-md:flex",
        "screen-[var(--spacing-ref-small)]:flex",
        "screen-(--spacing-ref-small):flex",
        "supports-[transition-duration:var(--duration-ref-fast)]:flex",
    ] {
        assert_eq!(
            kinds(&format!("const x=style('{utility}');"), &theme, &options),
            [LintKind::NoRestrictedThemeTokens],
            "{utility}"
        );
    }
    for utility in [
        "p-gutter",
        "duration-enter",
        "speed-enter:flex",
        "screen-[var(--spacing-gutter)]:flex",
        "supports-[transition-duration:var(--duration-enter)]:flex",
        "data-[token=var(--spacing-ref-small)]:flex",
        "[padding:var(--spacing-ref-runtime)]",
    ] {
        assert_eq!(
            kinds(&format!("const x=style('{utility}');"), &theme, &options),
            [] as [LintKind; 0],
            "{utility}"
        );
    }
    assert_eq!(
        kinds(
            "const x=style({'speed-ref-fast:':['flex','flex']});",
            &theme,
            &options
        )
        .iter()
        .filter(|kind| **kind == LintKind::NoRestrictedThemeTokens)
        .count(),
        1
    );
}

#[test]
fn utility_pairs_respect_css_contexts_source_choices_and_shorthand_policy() {
    for (code, expected) in [
        (
            "const x=style('p2 p2');",
            Some(LintKind::DuplicateUtilities),
        ),
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
            Some(LintKind::DuplicateUtilities),
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
        [LintKind::DuplicateUtilities]
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
        "{CSS} @stillwind {{ prefer-direct-styling:warn; }} @custom-variant parent (.parent &); @custom-variant has (&:has(>img)); @custom-variant before (&::before); @custom-variant legacy (&:after); @custom-variant part (&::part(label)); @custom-variant pseudos (&::before, &::after); @custom-variant child-before (&>span::before); @custom-variant mixed (&::before, &>span::after); @custom-variant list (:is(&,span)::before);"
    );
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
            kinds(
                &format!("const x=style('{utility}');"),
                &theme,
                &CompilerOptions::default()
            )
            .is_empty(),
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
            kinds(
                &format!("const x=style('{utility}');"),
                &theme,
                &CompilerOptions::default()
            ),
            [LintKind::PreferDirectStyling],
            "{utility}"
        );
    }
    assert_eq!(
        kinds(
            "// swlint-disable-next-line prefer-direct-styling\nconst x=style('child-before:p2');",
            &theme,
            &CompilerOptions::default(),
        ),
        [] as [LintKind; 0]
    );
}

#[test]
fn unit_domains_follow_tokens_shorthands_and_last_matching_rules() {
    let options = CompilerOptions {
        lint: LintOverrides {
            length_units: Some(LengthUnitsOverride {
                rules: Some(vec![
                    LengthUnitRule {
                        properties: vec!["padding*".into()],
                        disallow: vec!["px".into()],
                    },
                    LengthUnitRule {
                        properties: vec!["border-*-width".into()],
                        disallow: vec!["rem".into()],
                    },
                ]),
                ..LengthUnitsOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    let theme = ":root { @stillwind-slot theme; } @theme {--spacing-a:2px;--spacing-b:var(--spacing-a);} @utility p-* {padding:--value(--spacing-*,[length]);}";
    for code in [
        "const x=style('p-b');",
        "const x=style('[padding:calc(1rem_+_2px)]');",
        "const x=style('[border:1rem_solid_red]');",
    ] {
        assert!(
            kinds(code, theme, &options).contains(&LintKind::LengthUnits),
            "{code}"
        );
    }
    for code in [
        "const x=style('[padding:0px]');",
        "const x=style('[border:1px_solid_red]');",
        "const x=style('[padding:var(--unknown)]');",
    ] {
        assert!(
            kinds(code, theme, &options)
                .iter()
                .all(|kind| *kind != LintKind::LengthUnits),
            "{code}"
        );
    }
}

#[test]
fn unit_exceptions_override_broad_rules_for_shorthand_components() {
    let options = CompilerOptions {
        lint: LintOverrides {
            length_units: Some(LengthUnitsOverride {
                rules: Some(vec![
                    LengthUnitRule {
                        properties: vec!["*".into()],
                        disallow: vec!["px".into()],
                    },
                    LengthUnitRule {
                        properties: vec!["border-*-width".into()],
                        disallow: vec!["rem".into()],
                    },
                ]),
                ..LengthUnitsOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    assert_eq!(
        kinds("const x=style('[border:1px_solid_red]');", "", &options),
        [] as [LintKind; 0]
    );
    assert!(
        kinds("const x=style('[border:1rem_solid_red]');", "", &options)
            .contains(&LintKind::LengthUnits)
    );
}

#[test]
fn logical_policy_checks_properties_directional_values_and_asymmetric_shorthands() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_logical_properties: Some(LintSeverity::Warn),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    for candidate in [
        "[padding-left:1rem]",
        "[padding:1rem_2rem]",
        "[text-align:left]",
        "[float:right]",
        "[width:10rem]",
    ] {
        assert!(
            kinds(&format!("const x=style('{candidate}');"), "", &options)
                .contains(&LintKind::PreferLogicalProperties),
            "{candidate}"
        );
    }
    for candidate in [
        "[padding:1rem]",
        "[padding-inline:1rem]",
        "[text-align:start]",
        "[inline-size:10rem]",
        "[border:1px_solid_red]",
    ] {
        assert!(
            kinds(&format!("const x=style('{candidate}');"), "", &options).is_empty(),
            "{candidate}"
        );
    }
}

#[test]
fn direct_color_tokens_survive_inline_expansion_and_allow_semantic_aliases() {
    let options = CompilerOptions {
        lint: LintOverrides {
            no_restricted_theme_tokens: Some(RestrictedThemeTokensOverride {
                tokens: Some(vec!["--color-red-*".into()]),
                ..RestrictedThemeTokensOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    for mode in ["", "inline"] {
        let theme = format!(
            ":root {{ @stillwind-slot theme; }} @theme {mode} {{--color-red-500:red;--color-danger:var(--color-red-500);}} @utility text-* {{color:--value(--color-*,[color]);}}"
        );
        for utility in ["text-red-500", "[color:var(--color-red-500)]"] {
            assert!(
                kinds(&format!("const x=style('{utility}');"), &theme, &options)
                    .contains(&LintKind::NoRestrictedThemeTokens),
                "{mode}: {utility}"
            );
        }
        assert!(
            kinds("const x=style('text-danger');", &theme, &options).is_empty(),
            "{mode}"
        );
    }
    let theme = ":root { @stillwind-slot theme; } @theme inline {--color-red-500:red;} @utility tone-* { color:blue; } @utility tone-* { color:--value(--color-*); &:hover {color:--value(--color-*);} }";
    assert_eq!(
        kinds("const x=style('tone-red-500');", theme, &options),
        [LintKind::NoRestrictedThemeTokens]
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
            "\n// swlint-disable-next-line duplicate-utilities\nconst x=style('p2 p2');",
            CSS,
            &CompilerOptions::default()
        ),
        [] as [LintKind; 0]
    );
}

const VALUE_CSS: &str = "@theme inline { --color-primary:#123; --spacing-unit:1rem; }
@utility paint-* {color:--value(--color-*,[color]);}
@utility pad-* {padding:--value(--spacing-*,[length-percentage]);}
@utility mixed-* {color:--value([color]); margin:--value([length]); border-width:1px;}
@utility component {background:red; padding:13px;}
@custom-variant hover (&:hover);";

fn policy_options(mut lint: LintOverrides) -> CompilerOptions {
    lint.canonical_utilities = Some(CanonicalUtilitiesOverride {
        severity: Some(LintSeverity::Off),
        ..CanonicalUtilitiesOverride::default()
    });
    CompilerOptions {
        lint,
        ..CompilerOptions::default()
    }
}

#[test]
fn arbitrary_colors_track_accepted_captures_and_property_components() {
    let options = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    for utility in [
        "paint-[#abc]",
        "mixed-[red]",
        "hover:paint-[rgb(1_2_3)]",
        "[color:oklch(0.6_0.1_30)]",
        "[border:1px_solid_red]",
        "[background:linear-gradient(red,blue)]",
        "[box-shadow:0_0_2px_#fff]",
        "[fill:#abc]",
        "[color:var(--x,red)]",
    ] {
        let (source, result) =
            compile_with_options(&format!("const x=style('{utility}');"), VALUE_CSS, &options);
        assert!(
            result.diagnostics.is_empty(),
            "{utility}: {:?}",
            result.diagnostics
        );
        let findings = result
            .lint_findings
            .iter()
            .filter(|finding| finding.kind() == LintKind::NoArbitraryColors)
            .collect::<Vec<_>>();
        assert!(
            !findings.is_empty(),
            "{utility}: {:?}",
            result.lint_findings
        );
        assert_eq!(source.slice(findings[0].labels[0].span).unwrap(), utility);
    }
    for utility in [
        "paint-primary",
        "component",
        "paint-(--runtime)",
        "paint-[transparent]",
        "[color:currentColor]",
        "[color:inherit]",
        "[color:var(--runtime)]",
        "[color:color-mix(in_srgb,var(--a),var(--b))]",
        "[background:url(red.png)]",
        "[content:\"red\"]",
        "mixed-[13px]",
    ] {
        assert!(
            !kinds(&format!("const x=style('{utility}');"), VALUE_CSS, &options)
                .contains(&LintKind::NoArbitraryColors),
            "{utility}"
        );
    }
}

#[test]
fn arbitrary_spacing_defaults_to_margin_padding_and_gap_and_can_add_domains() {
    let options = policy_options(LintOverrides {
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride::default()),
        ..LintOverrides::default()
    });
    for utility in [
        "pad-[13px]",
        "mixed-[13px]",
        "[margin-inline:7%]",
        "[gap:calc(1rem_+_2px)]",
        "[padding:0_13px]",
    ] {
        let findings = kinds(&format!("const x=style('{utility}');"), VALUE_CSS, &options);
        assert_eq!(
            findings
                .iter()
                .filter(|kind| **kind == LintKind::NoArbitrarySpacing)
                .count(),
            1,
            "{utility}: {findings:?}"
        );
    }
    for utility in [
        "pad-unit",
        "component",
        "pad-(--runtime)",
        "pad-[0px]",
        "[margin:auto]",
        "[gap:var(--runtime)]",
        "[width:13px]",
        "[border-width:13px]",
        "[padding:var(--spacing-unit)]",
    ] {
        assert!(
            !kinds(&format!("const x=style('{utility}');"), VALUE_CSS, &options)
                .contains(&LintKind::NoArbitrarySpacing),
            "{utility}"
        );
    }
    let extra = policy_options(LintOverrides {
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride {
            properties: Some(vec!["WIDTH".into()]),
            ..ArbitrarySpacingOverride::default()
        }),
        ..LintOverrides::default()
    });
    assert!(
        kinds("const x=style('[width:13px]');", VALUE_CSS, &extra)
            .contains(&LintKind::NoArbitrarySpacing)
    );
}

#[test]
fn literal_inline_variables_are_checked_only_with_unambiguous_local_consumers() {
    let options = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride::default()),
        ..LintOverrides::default()
    });
    let (source, result) = compile_with_options(
        "const x=style('paint-(--fg) pad-(--gap)', {'--fg':'#abc','--gap':'13px'});",
        VALUE_CSS,
        &options,
    );
    for (kind, text) in [
        (LintKind::NoArbitraryColors, "'#abc'"),
        (LintKind::NoArbitrarySpacing, "'13px'"),
    ] {
        let finding = result
            .lint_findings
            .iter()
            .find(|finding| finding.kind() == kind)
            .expect("literal variable diagnostic");
        assert_eq!(source.slice(finding.labels[0].span).unwrap(), text);
    }
    for input in [
        "'paint-(--fg)',{'--fg':color}",
        "'paint-(--fg)', active && {'--fg':'#abc'}",
        "'paint-(--fg)',{'--fg':'#abc'},{'--fg':color}",
        "{'--unused':'#abc'}",
        "'paint-(--a)',{'--a':'var(--b)','--b':'var(--a)'}",
    ] {
        let findings = kinds(&format!("const x=style({input});"), VALUE_CSS, &options);
        assert!(
            !findings.contains(&LintKind::NoArbitraryColors),
            "{input}: {findings:?}"
        );
    }
    assert!(
        kinds(
            "const x=style('paint-(--a)',{'--a':'var(--b)','--b':'#abc'});",
            VALUE_CSS,
            &options
        )
        .contains(&LintKind::NoArbitraryColors)
    );
    let extra = policy_options(LintOverrides {
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride {
            properties: Some(vec!["border-width".into()]),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(
        kinds(
            "const x=style('[border:var(--line)]',{'--line':'2px solid red'});",
            VALUE_CSS,
            &extra
        )
        .contains(&LintKind::NoArbitrarySpacing)
    );
}

#[test]
fn class_property_policy_preserves_runtime_and_composition_channels() {
    let source = "const a=style({class:['legacy',enabled && 'active']}); const b=style(a,{'--progress':progress,'data-active':enabled});";
    let options = policy_options(LintOverrides {
        no_class_property: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    let (input, result) = compile_with_options(source, VALUE_CSS, &options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|finding| finding.kind() == LintKind::NoClassProperty)
            .count(),
        1
    );
    assert!(result.apply_to_source(&input).unwrap().contains("legacy"));
    assert!(
        !kinds(source, VALUE_CSS, &CompilerOptions::default()).contains(&LintKind::NoClassProperty)
    );
}

#[test]
fn utility_restrictions_check_variant_modifier_and_unresolved_spellings() {
    let options = policy_options(LintOverrides {
        no_restricted_utilities: Some(RestrictedUtilitiesOverride {
            utilities: Some(vec![RestrictedUtilityRule {
                pattern: "legacy*".into(),
                message: Some("Use component".into()),
            }]),
            ..RestrictedUtilitiesOverride::default()
        }),
        ..LintOverrides::default()
    });
    for input in ["'legacy'", "'hover:legacy!'", "{'hover:':'legacy/50'}"] {
        let (_, result) =
            compile_with_options(&format!("const x=style({input});"), VALUE_CSS, &options);
        let finding = result
            .lint_findings
            .iter()
            .find(|finding| finding.kind() == LintKind::NoRestrictedUtilities)
            .expect("restricted name");
        assert!(finding.to_string().contains("Use component"));
    }
    assert!(
        !kinds("const x=style({class:'legacy'});", VALUE_CSS, &options)
            .contains(&LintKind::NoRestrictedUtilities)
    );
}

#[test]
fn declaration_restrictions_and_importance_follow_resolved_utility_bodies() {
    let options = policy_options(LintOverrides {
        no_important: Some(LintSeverity::Warn),
        no_restricted_declarations: Some(RestrictedDeclarationsOverride {
            rules: Some(vec![
                RestrictedDeclarationRule {
                    properties: vec!["POSITION".into()],
                    values: vec!["FIXED".into()],
                    message: Some("Use a layer component".into()),
                },
                RestrictedDeclarationRule {
                    properties: vec!["transition-property".into()],
                    values: vec!["all".into()],
                    message: None,
                },
            ]),
            ..RestrictedDeclarationsOverride::default()
        }),
        ..LintOverrides::default()
    });
    let theme = format!(
        "{VALUE_CSS} @utility floating {{position:fixed!important}} @utility animated {{transition:all 1s}} @utility nested {{&>span{{position:fixed!important}}}}"
    );
    for utility in ["floating", "nested", "[position:fixed]!"] {
        let findings = kinds(&format!("const x=style('{utility}');"), &theme, &options);
        assert!(
            findings.contains(&LintKind::NoRestrictedDeclarations),
            "{utility}: {findings:?}"
        );
        assert!(
            findings.contains(&LintKind::NoImportant),
            "{utility}: {findings:?}"
        );
    }
    assert!(
        kinds("const x=style('animated');", &theme, &options)
            .contains(&LintKind::NoRestrictedDeclarations)
    );
    assert!(
        kinds("const x=style('hover:paint-primary!');", &theme, &options)
            .contains(&LintKind::NoImportant)
    );
    assert!(
        !kinds("const x=style('component');", &theme, &options).contains(&LintKind::NoImportant)
    );
}

#[test]
fn suppression_usage_includes_findings_detected_after_source_analysis() {
    let options = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        no_unused_lint_suppressions: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    let used = "\n// swlint-disable-next-line no-arbitrary-colors\nconst x=style('paint-[#abc]');";
    assert_eq!(kinds(used, VALUE_CSS, &options), [] as [LintKind; 0]);
    let unused =
        "\n// swlint-disable-next-line no-arbitrary-colors\nconst x=style('paint-primary');";
    assert_eq!(
        kinds(unused, VALUE_CSS, &options),
        [LintKind::NoUnusedLintSuppressions]
    );
    let invalid = "\n// swlint-disable-next-line invalid-rule\nconst x=style('paint-primary');";
    assert_eq!(
        kinds(invalid, VALUE_CSS, &options),
        [LintKind::ValidLintSuppressions]
    );
    let off = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Off),
        no_unused_lint_suppressions: Some(LintSeverity::Warn),
        ..LintOverrides::default()
    });
    assert_eq!(kinds(unused, VALUE_CSS, &off), [] as [LintKind; 0]);
}

#[test]
fn canonical_suggestions_do_not_introduce_restricted_utility_names() {
    let options = CompilerOptions {
        lint: LintOverrides {
            no_restricted_utilities: Some(RestrictedUtilitiesOverride {
                utilities: Some(vec![RestrictedUtilityRule {
                    pattern: "paint-primary".into(),
                    message: None,
                }]),
                ..RestrictedUtilitiesOverride::default()
            }),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    };
    assert!(
        !kinds("const x=style('paint-[#123]');", VALUE_CSS, &options)
            .contains(&LintKind::CanonicalUtilities)
    );
}
