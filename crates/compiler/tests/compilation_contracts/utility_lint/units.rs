//! Checks length units and logical CSS property policies.

use super::*;

#[test]
fn unit_domains_follow_tokens_shorthands_and_last_matching_rules() {
    let options = CompilerOptions {
        lint: LintOverrides {
            no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
                rules: Some(vec![
                    RestrictedLengthUnitRule {
                        properties: vec!["padding*".into()],
                        disallow: vec!["px".into()],
                    },
                    RestrictedLengthUnitRule {
                        properties: vec!["border-*-width".into()],
                        disallow: vec!["rem".into()],
                    },
                ]),
                ..RestrictedLengthUnitsOverride::default()
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
            kinds(code, theme, &options).contains(&LintKind::NoRestrictedLengthUnits),
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
                .all(|kind| *kind != LintKind::NoRestrictedLengthUnits),
            "{code}"
        );
    }
}

#[test]
fn unit_exceptions_override_broad_rules_for_shorthand_components() {
    let options = CompilerOptions {
        lint: LintOverrides {
            no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
                rules: Some(vec![
                    RestrictedLengthUnitRule {
                        properties: vec!["*".into()],
                        disallow: vec!["px".into()],
                    },
                    RestrictedLengthUnitRule {
                        properties: vec!["border-*-width".into()],
                        disallow: vec!["rem".into()],
                    },
                ]),
                ..RestrictedLengthUnitsOverride::default()
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
            .contains(&LintKind::NoRestrictedLengthUnits)
    );
}

#[test]
fn logical_policy_checks_properties_directional_values_and_asymmetric_shorthands() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_logical_properties: Some(LogicalPropertiesOverride {
                severity: Some(LintSeverity::Warn),
                ..LogicalPropertiesOverride::default()
            }),
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
fn implementation_declarations_are_optional_without_disabling_source_policies() {
    let theme = "@utility icon (implementation) { width:max-content; height:1em; @media (width > 40rem) { height:2em; } &::before { width:1em; } } @utility glow (implementation) { box-shadow:0 1em oklch(0.7 0.1 40); } @custom-variant hover (&:hover);";
    for check_implementation in [false, true] {
        let options = policy_options(LintOverrides {
            prefer_logical_properties: Some(LogicalPropertiesOverride {
                check_implementation: Some(check_implementation),
                ..Default::default()
            }),
            no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
                check_implementation: Some(check_implementation),
                rules: Some(vec![RestrictedLengthUnitRule {
                    properties: vec!["*".into()],
                    disallow: vec!["em".into(), "px".into()],
                }]),
                ..Default::default()
            }),
            no_important: Some(ImportantOverride {
                severity: Some(LintSeverity::Warn),
                ..Default::default()
            }),
            ..Default::default()
        });
        for candidate in ["icon", "hover:icon"] {
            let found = kinds(&format!("const x=style('{candidate}');"), theme, &options);
            for kind in [
                LintKind::PreferLogicalProperties,
                LintKind::NoRestrictedLengthUnits,
            ] {
                assert_eq!(
                    found.contains(&kind),
                    check_implementation,
                    "{candidate}: {found:?}"
                );
            }
        }
        assert_eq!(
            kinds("const x=style('glow');", theme, &options)
                .contains(&LintKind::NoRestrictedLengthUnits),
            check_implementation
        );
        let found = kinds("const x=style('icon!');", theme, &options);
        assert_eq!(
            found
                .iter()
                .filter(|kind| **kind == LintKind::NoImportant)
                .count(),
            1
        );
        assert!(
            kinds("const x=style('[width:20px]');", theme, &options)
                .contains(&LintKind::NoRestrictedLengthUnits)
        );
    }
}

#[test]
fn implementation_headers_preserve_captures_and_unmarked_extensions() {
    let options = policy_options(LintOverrides {
        prefer_logical_properties: Some(LogicalPropertiesOverride::default()),
        no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
            rules: Some(vec![RestrictedLengthUnitRule {
                properties: vec!["*".into()],
                disallow: vec!["px".into(), "em".into()],
            }]),
            ..Default::default()
        }),
        ..Default::default()
    });
    let theme = "@theme inline { --spacing-big:20px; }
        @utility shape-* (implementation) { width:--value(--spacing-*, [length]); height:1em; }
        @utility modified (implementation) { height:1em; width:--modifier([length]); }
        @utility same (implementation) { width:20px; &::before { height:20px; } }
        @utility same { width:20px; &::before { height:20px; } }";
    for candidate in ["shape-[20px]", "shape-big", "modified/[20px]", "same"] {
        let (_, result) =
            compile_with_options(&format!("const x=style('{candidate}');"), theme, &options);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(
            result
                .lint_findings
                .iter()
                .any(|finding| finding.kind() == LintKind::PreferLogicalProperties),
            "{candidate}"
        );
        assert!(
            result
                .lint_findings
                .iter()
                .any(|finding| finding.kind() == LintKind::NoRestrictedLengthUnits),
            "{candidate}"
        );
    }
}

#[test]
fn implementation_classification_keeps_declaration_bans_and_authored_priority() {
    let options = policy_options(LintOverrides {
        no_important: Some(ImportantOverride {
            severity: Some(LintSeverity::Warn),
            ..Default::default()
        }),
        no_restricted_declarations: Some(RestrictedDeclarationsOverride {
            rules: Some(vec![RestrictedDeclarationRule {
                properties: vec!["width".into()],
                values: vec!["*".into()],
                message: None,
            }]),
            ..Default::default()
        }),
        ..Default::default()
    });
    let found = kinds(
        "const x=style('icon');",
        "@utility icon (implementation) { width:20px!important; }",
        &options,
    );
    assert!(found.contains(&LintKind::NoImportant));
    assert!(found.contains(&LintKind::NoRestrictedDeclarations));
}

#[test]
fn implementation_classifications_follow_retained_rules_and_variant_declarations() {
    let mut options = policy_options(LintOverrides {
        prefer_logical_properties: Some(LogicalPropertiesOverride::default()),
        no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
            rules: Some(vec![RestrictedLengthUnitRule {
                properties: vec!["*".into()],
                disallow: vec!["em".into()],
            }]),
            ..Default::default()
        }),
        ..Default::default()
    });
    options.use_css_if = true;
    let theme = r"@stillwind-variant if\(*\) (value: [if-condition]) { @stillwind-intrinsic builtin-if-variant; }
        @utility icon (implementation) { display:block; &::before { height:1em; } }
        @utility own (implementation) { height:1em; }
        @variant height-* { & { height:--value([length]); @slot; } }
        @utility media { @media (width > 40rem) { height:1em; } }
        @utility media (implementation) { @media (width > 40rem) { height:1em; } }";
    for candidate in ["if(--active):own", "height-[1em]:own", "media"] {
        let found = kinds(&format!("const x=style('{candidate}');"), theme, &options);
        assert!(found.is_empty(), "{candidate}: {found:?}");
    }
    for candidate in [
        "height-[1em]:icon",
        "if(--active):height-[1em]:icon",
        "if(--active):[height:1em]",
    ] {
        let found = kinds(&format!("const x=style('{candidate}');"), theme, &options);
        assert!(
            found.contains(&LintKind::PreferLogicalProperties),
            "{candidate}: {found:?}"
        );
        assert!(
            found.contains(&LintKind::NoRestrictedLengthUnits),
            "{candidate}: {found:?}"
        );
    }
}
