//! Checks length units and logical CSS property policies.

use super::*;

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
