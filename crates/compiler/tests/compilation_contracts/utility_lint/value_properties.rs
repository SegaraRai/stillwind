//! Value policies for custom properties with declared receiving properties.

use stillwind_diagnostics::LintDiagnostic;

use super::*;

#[test]
fn builtin_value_properties_enforce_units_once_under_the_receiving_property() {
    let options = policy_options(LintOverrides {
        no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
            rules: Some(vec![RestrictedLengthUnitRule {
                properties: vec!["*".into()],
                disallow: vec!["px".into()],
            }]),
            ..Default::default()
        }),
        ..Default::default()
    });
    let css = with_defaults(
        "@theme { --spacing-test:13px; --blur-test:13px; --shadow-test:0 0 13px red; }",
    );
    let fixture = CompilationFixture::new(&css, &options);
    for (candidate, property) in [
        ("border-spacing-[13px]", "border-spacing"),
        ("border-spacing-x-[13px]", "border-spacing"),
        ("border-spacing-y-[13px]", "border-spacing"),
        ("translate-x-[13px]", "translate"),
        ("translate-y-[13px]", "translate"),
        ("translate-z-[13px]", "translate"),
        ("translate-x-test", "translate"),
        ("blur-[13px]", "filter"),
        ("blur-test", "filter"),
        ("drop-shadow-[0_0_13px_red]", "filter"),
        ("backdrop-blur-[13px]", "backdrop-filter"),
        ("from-[13px]", "background-image"),
        ("via-[13px]", "background-image"),
        ("to-[13px]", "background-image"),
        ("mask-t-from-[13px]", "mask-image"),
        ("mask-linear-from-[13px]", "mask-image"),
        ("mask-radial-from-[13px]", "mask-image"),
        ("mask-radial-at-[13px_20px]", "mask-position"),
        ("[--tw-mask-radial-position:13px_20px]", "mask-image"),
        ("shadow-[0_0_13px_red]", "box-shadow"),
        ("shadow-test", "box-shadow"),
        ("inset-shadow-[0_0_13px_red]", "box-shadow"),
        ("ring-[13px]", "box-shadow"),
        ("ring-offset-13", "box-shadow"),
        ("tracking-[13px]", "letter-spacing"),
        ("leading-[13px]", "line-height"),
    ] {
        let (_, result) = fixture.compile(&format!("const x=style('{candidate}');"));
        assert!(
            result.diagnostics.is_empty(),
            "{candidate}: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.lint_findings.len(),
            1,
            "{candidate}: {:?}",
            result.lint_findings
        );
        assert!(
            matches!(&result.lint_findings[0].diagnostic, LintDiagnostic::RestrictedLengthUnit { property: actual, unit } if actual == property && unit == "px"),
            "{candidate}: {:?}",
            result.lint_findings
        );
    }
    let (_, result) = fixture.compile("export const frames=style.keyframes({from:'translate-x-[13px] blur-[13px] backdrop-blur-[13px]'});");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let mut properties = result
        .lint_findings
        .iter()
        .map(|finding| match &finding.diagnostic {
            LintDiagnostic::RestrictedLengthUnit { property, .. } => property.as_str(),
            diagnostic => panic!("unexpected keyframe diagnostic: {diagnostic:?}"),
        })
        .collect::<Vec<_>>();
    properties.sort_unstable();
    assert_eq!(properties, ["backdrop-filter", "filter", "translate"]);
}

#[test]
fn builtin_value_properties_check_colors_without_classifying_control_values_as_spacing() {
    let options = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride {
            properties: Some(vec![
                "box-shadow".into(),
                "filter".into(),
                "mask-image".into(),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    });
    let fixture = CompilationFixture::new(&with_defaults(""), &options);
    for (candidate, property) in [
        ("mask-t-from-[red]", "mask-image"),
        ("[--tw-mask-top-from-color:red]", "mask-image"),
        ("drop-shadow-[red]", "filter"),
        ("[--tw-drop-shadow-color:red]", "filter"),
        (
            "[mask-image:linear-gradient(red,transparent)]",
            "mask-image",
        ),
        ("[filter:drop-shadow(0_0_0_red)]", "filter"),
        (
            "[backdrop-filter:drop-shadow(0_0_0_red)]",
            "backdrop-filter",
        ),
        ("[--tw-scrollbar-thumb:red]", "scrollbar-color"),
        ("shadow-[red]", "box-shadow"),
        ("text-shadow-[red]", "text-shadow"),
        ("from-[red]", "background-image"),
    ] {
        let (_, result) = fixture.compile(&format!("const x=style('{candidate}');"));
        assert!(
            result.diagnostics.is_empty(),
            "{candidate}: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.lint_findings.len(),
            1,
            "{candidate}: {:?}",
            result.lint_findings
        );
        assert!(
            matches!(&result.lint_findings[0].diagnostic, LintDiagnostic::ArbitraryColor { property: actual, .. } if actual == property),
            "{candidate}: {:?}",
            result.lint_findings
        );
    }
    for candidate in [
        "mask-t-from-red-500",
        "drop-shadow-red-500",
        "[--tw-shadow-alpha:50%]",
        "[--tw-inset-shadow-alpha:50%]",
        "[--tw-drop-shadow-alpha:50%]",
        "[--tw-text-shadow-alpha:50%]",
        "[--tw-mask-top-from-color:var(--paint)]",
        "[mask-image:url(/red.svg)]",
        "[filter:blur(0px)]",
    ] {
        assert_eq!(
            kinds_in(&format!("const x=style('{candidate}');"), &fixture),
            [] as [LintKind; 0],
            "{candidate}"
        );
    }
}

#[test]
fn value_properties_check_every_target_and_deduplicate_repeated_declarations() {
    let css = r#"
        :root { @stillwind-slot theme; }
        @stillwind-property --size {
            syntax: "*";
            inherits: false;
            value-for: width, height, width;
        }
        @theme { --spacing-bad:20px; }
        @utility box-* (implementation) {
            --size:--value(--spacing-*, [length]);
            @media print { --size:--value(--spacing-*, [length]); }
            &::before { --size:--value(--spacing-*, [length]); }
        }
        @utility nested-* { @media print { --size:--value([length]); } }
        @utility child-* { &::before { --size:--value([length]); } }
        @variant extra { & { --size:20px; @slot; } }
        @utility empty { display:block; }
    "#;
    for width_disallow in [vec!["px".into()], vec![]] {
        let options = policy_options(LintOverrides {
            prefer_logical_properties: Some(LogicalPropertiesOverride {
                severity: Some(LintSeverity::Warn),
                ..Default::default()
            }),
            no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
                rules: Some(vec![
                    RestrictedLengthUnitRule {
                        properties: vec!["height".into()],
                        disallow: vec!["px".into()],
                    },
                    RestrictedLengthUnitRule {
                        properties: vec!["width".into()],
                        disallow: width_disallow.clone(),
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        });
        for candidate in [
            "box-[20px]",
            "box-bad",
            "nested-[20px]",
            "child-[20px]",
            "[--size:20px]",
            "extra:empty",
        ] {
            let (_, result) =
                compile_with_options(&format!("const x=style('{candidate}');"), css, &options);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            let mut properties = result
                .lint_findings
                .iter()
                .map(|finding| match &finding.diagnostic {
                    LintDiagnostic::RestrictedLengthUnit { property, unit } => {
                        assert_eq!(unit, "px");
                        property.as_str()
                    }
                    diagnostic => panic!("unexpected diagnostic for {candidate}: {diagnostic:?}"),
                })
                .collect::<Vec<_>>();
            properties.sort_unstable();
            assert_eq!(
                properties,
                if width_disallow.is_empty() {
                    vec!["height"]
                } else {
                    vec!["height", "width"]
                },
                "{candidate}"
            );
        }
    }
}

#[test]
fn arbitrary_spacing_uses_mapped_domains_and_keeps_named_values_valid() {
    let options = policy_options(LintOverrides {
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride {
            properties: Some(vec!["width".into()]),
            ..Default::default()
        }),
        ..Default::default()
    });
    let css = r"
        :root { @stillwind-slot theme; }
        @stillwind-property --size { value-for: padding, width, padding; }
        @theme { --spacing-good:2rem; }
        @utility box-* { --size:--value(--spacing-*, [length]); }
    ";
    for candidate in ["box-[13px]", "[--size:13px]"] {
        let (_, result) =
            compile_with_options(&format!("const x=style('{candidate}');"), css, &options);
        let properties = result
            .lint_findings
            .iter()
            .map(|finding| match &finding.diagnostic {
                LintDiagnostic::ArbitrarySpacing { property, .. } => property.as_str(),
                diagnostic => panic!("unexpected diagnostic: {diagnostic:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(properties, ["padding", "width"]);
    }
    for candidate in ["box-good", "box-(--runtime)", "box-[0px]"] {
        assert_eq!(
            kinds(&format!("const x=style('{candidate}');"), css, &options),
            [] as [LintKind; 0]
        );
    }
    let colors = policy_options(LintOverrides {
        no_arbitrary_colors: Some(LintSeverity::Warn),
        ..Default::default()
    });
    assert_eq!(
        kinds(
            "const x=style('[--paint:red]');",
            "@stillwind-property --paint { value-for: color; }",
            &colors
        ),
        [LintKind::NoArbitraryColors]
    );
}

#[test]
fn value_properties_preserve_declaration_restrictions_and_registration_precedence() {
    let options = policy_options(LintOverrides {
        no_restricted_declarations: Some(RestrictedDeclarationsOverride {
            rules: Some(vec![RestrictedDeclarationRule {
                properties: vec!["--size".into()],
                values: vec!["*".into()],
                message: None,
            }]),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert_eq!(
        kinds(
            "const x=style('[--size:13px]');",
            "@stillwind-property --size { value-for: height; }",
            &options
        ),
        [LintKind::NoRestrictedDeclarations]
    );

    let spacing = policy_options(LintOverrides {
        no_arbitrary_spacing: Some(ArbitrarySpacingOverride::default()),
        ..Default::default()
    });
    let css = "@stillwind-property --Size { value-for: padding; } @stillwind-property --size { value-for: height; } @utility box-* { --Size:--value([length]); }";
    for candidate in ["box-[13px]", "[--Size:13px]"] {
        assert_eq!(
            kinds(&format!("const x=style('{candidate}');"), css, &spacing),
            [LintKind::NoArbitrarySpacing]
        );
    }
    assert_eq!(
        kinds("const x=style('[--size:13px]');", css, &spacing),
        [] as [LintKind; 0]
    );
    for last in ["value-for: height;", "syntax: '*';"] {
        let css = format!(
            "@stillwind-property --size {{ value-for: padding; }} @stillwind-property --size {{ {last} }}"
        );
        assert_eq!(
            kinds("const x=style('[--size:13px]');", &css, &spacing),
            [] as [LintKind; 0]
        );
    }
}
