//! Checks source value, utility, declaration, and token restrictions.

use super::*;

#[test]
fn restricted_theme_tokens_cover_non_color_values_and_variant_selections() {
    let theme = "@theme inline {--spacing-ref-small:1rem; --spacing-gutter:var(--spacing-ref-small); --duration-ref-fast:100ms; --duration-enter:var(--duration-ref-fast); --breakpoint-md:48rem; }
@utility p-* {padding:--value(--spacing-*,[length]);} @utility duration-* {transition-duration:--value(--duration-*,[time]);} @utility flex {display:flex;}
@variant speed-* { & {transition-duration:--value(--duration-*); @slot;} }
@variant screen-* { @media (width>=--value([length])) {@slot;} }
@variant choose-* { @media (width>=--value(--spacing-*,[length])) { @supports (display:--value([color])) {@slot;} } }
@variant choose-* { @media (width>=--value(--breakpoint-*,[length])) {@slot;} }";
    let theme = with_defaults(theme);
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
    let fixture = CompilationFixture::new(&theme, &options);
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
            kinds_in(&format!("const x=style('{utility}');"), &fixture),
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
            kinds_in(&format!("const x=style('{utility}');"), &fixture),
            [] as [LintKind; 0],
            "{utility}"
        );
    }
    assert_eq!(
        kinds_in(
            "const x=style({'speed-ref-fast:':['flex','flex']});",
            &fixture,
        )
        .iter()
        .filter(|kind| **kind == LintKind::NoRestrictedThemeTokens)
        .count(),
        1
    );
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
