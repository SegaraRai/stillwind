//! Shared source builders and result helpers for these contracts.

use super::*;

pub(super) const CSS: &str = "@utility p2 {padding:0.5rem} @utility p4 {padding:1rem} @utility px4 {padding-inline:1rem} @utility ps2 {padding-inline-start:0.5rem} @utility pl4 {padding-left:1rem} @utility py4 {padding-block:1rem} @custom-variant hover (&:hover); @custom-variant child (&>span); @utility child { &>span { color:red; } }";

pub(super) fn kinds(code: &str, theme: &str, options: &CompilerOptions) -> Vec<LintKind> {
    kinds_in(code, &CompilationFixture::new(theme, options))
}

pub(super) fn kinds_in(code: &str, fixture: &CompilationFixture) -> Vec<LintKind> {
    let (_, result) = fixture.compile(code);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.lint_findings.iter().map(LintFinding::kind).collect()
}

pub(super) const VALUE_CSS: &str = "@theme inline { --color-primary:#123; --spacing-unit:1rem; }
@utility paint-* {color:--value(--color-*,[color]);}
@utility pad-* {padding:--value(--spacing-*,[length-percentage]);}
@utility mixed-* {color:--value([color]); margin:--value([length]); border-width:1px;}
@utility component {background:red; padding:13px;}
@custom-variant hover (&:hover);";

pub(super) fn policy_options(mut lint: LintOverrides) -> CompilerOptions {
    lint.prefer_canonical_utilities = Some(CanonicalUtilitiesOverride {
        severity: Some(LintSeverity::Off),
        ..CanonicalUtilitiesOverride::default()
    });
    CompilerOptions {
        lint,
        ..CompilerOptions::default()
    }
}
