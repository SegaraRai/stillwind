//! Checks overrides by conditions, bases, and recipe options that leave properties of
//! the overridden utility in place.

use super::*;

/// `txt-*` sets a line height only with a theme value, as `text-*` does.
const OVERRIDE_CSS: &str = ":root { @stillwind-slot theme; }
@theme inline { --txt-sm:0.875rem; }
@utility txt-* {font-size:--value(--txt-*); line-height:1.25rem;}
@utility txt-* {font-size:--value([length]);}
@utility lead-* {line-height:--value([length]);}
@utility px-* {padding-inline:--value([length]);}
@utility pl-* {padding-left:--value([length]);}
@custom-variant md (@media (width >= 48rem));";

fn options() -> CompilerOptions {
    policy_options(LintOverrides::default())
}

#[test]
fn later_layers_report_properties_that_remain_from_the_overridden_utility() {
    for (code, expected) in [
        (
            "const x=style('txt-sm', big && 'txt-[13px]');",
            vec![LintKind::PartialUtilityOverride],
        ),
        (
            "const base=style('txt-sm'); const x=style(base, 'txt-[13px]');",
            vec![LintKind::PartialUtilityOverride],
        ),
        (
            "const r=style.recipe('txt-sm', {variants:{size:{small:'txt-[13px]'}}});",
            vec![LintKind::PartialUtilityOverride],
        ),
        (
            "const r=style.recipe('px-[1rem]', {slots:{label:'txt-sm'}, compoundVariants:[{size:'small', class:{label:'txt-[13px]'}}], variants:{size:{small:''}}});",
            vec![LintKind::PartialUtilityOverride],
        ),
        (
            "const r=style.recipe('px-[1rem]', {slots:{label:'txt-sm'}, variants:{size:{small:'txt-[13px]'}}});",
            vec![],
        ),
        (
            "const x=style('txt-sm', big && 'txt-[13px] lead-[1rem]');",
            vec![],
        ),
        ("const x=style('px-[1rem]', big && 'pl-[2rem]');", vec![]),
        ("const x=style('txt-sm md:txt-[13px]');", vec![]),
        (
            "const x=style(big && 'txt-[13px]', 'txt-sm');",
            vec![LintKind::ConflictingUtilities],
        ),
    ] {
        assert_eq!(kinds(code, OVERRIDE_CSS, &options()), expected, "{code}");
    }
}

#[test]
fn a_utility_under_more_conditions_overrides_without_a_conflict() {
    assert_eq!(
        kinds("const x=style('p2', flag && 'p4');", CSS, &options()),
        [] as [LintKind; 0]
    );
    assert_eq!(
        kinds("const x=style(flag && 'p4', 'p2');", CSS, &options()),
        [LintKind::ConflictingUtilities]
    );
}

#[test]
fn findings_name_the_remaining_properties_at_the_overriding_utility() {
    let code = "const x=style('txt-sm', big && 'txt-[13px]');";
    let (source, result) =
        super::super::support::compile_with_options(code, OVERRIDE_CSS, &options());
    let [finding] = result.lint_findings.as_slice() else {
        panic!("{:?}", result.lint_findings);
    };
    assert_eq!(
        finding.diagnostic.to_string(),
        "`txt-[13px]` overrides `txt-sm`, but `txt-sm` also sets `line-height`, which `txt-[13px]` does not set"
    );
    let labels = finding
        .info
        .labels
        .iter()
        .map(|label| source.slice(label.span).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(labels, ["txt-[13px]", "txt-sm"]);
}
