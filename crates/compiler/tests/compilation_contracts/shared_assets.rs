//! Shared-asset output, token policies, and independent compilation selection.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::{
    CanonicalUtilitiesOverride, LintDiagnostic, LintOverrides, LintSeverity,
    RestrictedLengthUnitRule, RestrictedLengthUnitsOverride, RestrictedThemeTokensOverride,
};

use super::support::CompilationFixture;

#[test]
fn shared_assets_are_selected_per_output_and_keep_consumer_variants_and_importance() {
    let fixture = CompilationFixture::new("@theme default asset {--image:url(data:image/svg+xml,a); --unused:url(data:image/svg+xml,unused);}
        @theme {--*:initial;}
        :root, :host {@stillwind-slot theme;}
        @utility icon {mask-image:var(--image);}
        @variant hover {&:hover {@slot;}}", &CompilerOptions::default());
    for code in [
        "export const a=style('icon hover:icon!');",
        "export const b=style('icon');",
    ] {
        let (_, result) = fixture.compile(code);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.css.matches("--image:").count(), 1, "{}", result.css);
        assert!(
            result.css.contains("--image:url(data:image/svg+xml,a);"),
            "{}",
            result.css
        );
        assert!(!result.css.contains("--unused:"));
    }
    let (_, empty) = fixture.compile("export const empty=style('');");
    assert!(!empty.css.contains("--image:"));
    let (_, important) = fixture.compile("export const a=style('hover:icon!');");
    assert!(
        important.css.contains("mask-image:var(--image)!important"),
        "{}",
        important.css
    );
    assert!(
        !important
            .css
            .contains("--image:url(data:image/svg+xml,a)!important")
    );
}

#[test]
fn shared_assets_keep_inline_static_and_transitive_output_policies() {
    let fixture = CompilationFixture::new(
        "@theme {--color:red;} @theme asset {--paint:linear-gradient(var(--color),transparent);}
        @theme asset inline {--image:var(--paint);}
        @theme asset static {--retained:url(data:image/svg+xml,static);}
        :root {@stillwind-slot theme;}
        @utility icon {mask-image:var(--image);}",
        &CompilerOptions::default(),
    );
    let (_, result) = fixture.compile("export const icon=style('icon');");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    for text in [
        "mask-image:var(--paint)",
        "--paint:linear-gradient(var(--color),transparent)",
        "--color:red",
        "--retained:url(data:image/svg+xml,static)",
    ] {
        assert!(result.css.contains(text), "{text}: {}", result.css);
    }
    assert!(!result.css.contains("--image:"));
    let (_, empty) = fixture.compile("export const empty=style('');");
    assert!(empty.css.contains("--retained:"));
    assert!(!empty.css.contains("--paint:"));
}

#[test]
fn asset_roles_exclude_token_rules_without_disabling_unit_policies() {
    let options = CompilerOptions {
        lint: LintOverrides {
            prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
                severity: Some(LintSeverity::Off),
                ..Default::default()
            }),
            no_restricted_theme_tokens: Some(RestrictedThemeTokensOverride {
                tokens: Some(vec!["--*".into()]),
                check_implementation: Some(true),
                ..Default::default()
            }),
            no_restricted_length_units: Some(RestrictedLengthUnitsOverride {
                rules: Some(vec![RestrictedLengthUnitRule {
                    properties: vec!["width".into()],
                    disallow: vec!["px".into()],
                }]),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let fixture = CompilationFixture::new(
        "@theme {--color-ref:red;}
        @theme asset {--color-image:url(data:image/svg+xml,a); --distance:13px;}
        :root {@stillwind-slot theme;}
        @utility icon (implementation) {mask-image:var(--color-image); color:var(--color-ref);}
        @utility width {width:var(--distance);}",
        &options,
    );
    let (_, result) = fixture.compile("export const icon=style('icon width');");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.lint_findings.len(), 2, "{:?}", result.lint_findings);
    assert!(result.lint_findings.iter().any(|finding| matches!(&finding.diagnostic, LintDiagnostic::RestrictedThemeToken {token} if token == "--color-ref")));
    assert!(result.lint_findings.iter().any(|finding| matches!(&finding.diagnostic, LintDiagnostic::RestrictedLengthUnit {property,unit} if property == "width" && unit == "px")));
}
