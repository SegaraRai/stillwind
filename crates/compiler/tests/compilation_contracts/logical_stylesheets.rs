//! Logical declarations of the axis utilities from `stillwind/logical/*.css`.

use stillwind_compiler::CompilerOptions;
use stillwind_design_system::UtilityOrderMode;
use stillwind_diagnostics::{LintKind, LintOverrides, LintSeverity, LogicalPropertiesOverride};
use stillwind_test_support::PACKAGE_CSS;

use super::compile;
use super::support::compile_with_options;

const LOGICAL_OVERFLOW: &str = include_str!("../../../../packages/core/css/logical/overflow.css");

fn stylesheet(project: &str) -> String {
    format!("{PACKAGE_CSS}\n{LOGICAL_OVERFLOW}\n{project}")
}

#[test]
fn axis_utilities_keep_their_names_and_write_logical_properties() {
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
    let (_, result) = compile_with_options(
        "export const x = style('overflow-x-auto overflow-y-hidden');",
        &stylesheet(""),
        &options,
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(result.lint_findings.is_empty(), "{result:#?}");
    assert!(
        result.css.contains("overflow-inline:auto"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("overflow-block:hidden"),
        "{}",
        result.css
    );
    assert!(!result.css.contains("overflow-x:"), "{}", result.css);
    assert!(!result.css.contains("overflow-y:"), "{}", result.css);
}

#[test]
fn axis_utilities_keep_their_position_in_every_order_mode() {
    let css = stylesheet("@utility mine { overflow:hidden; }");
    for utility_order in [UtilityOrderMode::default(), UtilityOrderMode::Tailwind] {
        let options = CompilerOptions {
            utility_order,
            ..CompilerOptions::default()
        };
        for shorthand in [
            "overflow-hidden",
            "truncate",
            "sr-only",
            "[overflow:hidden]",
        ] {
            let (_, result) = compile_with_options(
                &format!("export const x = style('overflow-x-auto {shorthand}');"),
                &css,
                &options,
            );
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert!(
                result.css.find("overflow:hidden").unwrap()
                    < result.css.find("overflow-inline:auto").unwrap(),
                "{shorthand}: {}",
                result.css
            );
        }
    }
    let (_, result) = compile("export const x = style('overflow-x-auto mine');", &css);
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(
        result.css.find("overflow-inline:auto").unwrap()
            < result.css.find("overflow:hidden").unwrap(),
        "{}",
        result.css
    );
}

#[test]
fn replacements_reach_keyframes_and_arbitrary_properties_stay_as_written() {
    let css = stylesheet("");
    for (code, declaration) in [
        (
            "export const a = style.keyframes({from:'overflow-x-hidden'});",
            "overflow-inline:hidden",
        ),
        (
            "export const x = style('[overflow-x:auto]');",
            "overflow-x:auto",
        ),
        (
            "export const x = style('[overflow-block:clip]');",
            "overflow-block:clip",
        ),
    ] {
        let (_, result) = compile(code, &css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(
            result
                .lint_findings
                .iter()
                .all(|finding| finding.kind() != LintKind::UnknownUtility),
            "{code}: {result:#?}"
        );
        assert!(result.css.contains(declaration), "{}", result.css);
    }
}

#[test]
fn overscroll_axis_utilities_write_logical_properties_after_the_shorthand() {
    const LOGICAL_OVERSCROLL: &str =
        include_str!("../../../../packages/core/css/logical/overscroll.css");
    let css = format!("{PACKAGE_CSS}\n{LOGICAL_OVERSCROLL}");
    for utility_order in [UtilityOrderMode::default(), UtilityOrderMode::Tailwind] {
        let options = CompilerOptions {
            utility_order,
            ..CompilerOptions::default()
        };
        let (_, result) = compile_with_options(
            "export const x = style('overscroll-x-contain overscroll-y-none overscroll-auto');",
            &css,
            &options,
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        let shorthand = result.css.find("overscroll-behavior:auto").unwrap();
        assert!(
            shorthand
                < result
                    .css
                    .find("overscroll-behavior-inline:contain")
                    .unwrap(),
            "{}",
            result.css
        );
        assert!(
            shorthand < result.css.find("overscroll-behavior-block:none").unwrap(),
            "{}",
            result.css
        );
    }
}

#[test]
fn divide_y_utilities_write_logical_properties_and_keep_reverse_and_style_utilities() {
    const LOGICAL_DIVIDE: &str = include_str!("../../../../packages/core/css/logical/divide.css");
    let css = format!("{PACKAGE_CSS}\n{LOGICAL_DIVIDE}");
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
    for candidates in [
        "divide-y",
        "divide-y-2",
        "divide-y-[3px]",
        "divide-y-[initial]",
    ] {
        let (_, result) = compile_with_options(
            &format!("export const x = style('{candidates}');"),
            &css,
            &options,
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.is_empty(), "{result:#?}");
        assert!(result.css.contains("border-block-"), "{}", result.css);
        assert!(!result.css.contains("border-top"), "{}", result.css);
        assert!(!result.css.contains("border-bottom"), "{}", result.css);
    }
    let (_, result) = compile_with_options(
        "export const x = style('divide-dashed divide-y-reverse divide-y');",
        &css,
        &options,
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(
        result.css.find("--tw-divide-y-reverse:0").unwrap()
            < result.css.find("--tw-divide-y-reverse:1").unwrap(),
        "{}",
        result.css
    );
    assert!(
        result.css.find("border-block-style").unwrap()
            < result.css.find("border-style:dashed").unwrap(),
        "{}",
        result.css
    );
}

#[test]
fn snap_and_resize_axes_write_logical_values() {
    const LOGICAL_SNAP: &str = include_str!("../../../../packages/core/css/logical/snap.css");
    const LOGICAL_RESIZE: &str = include_str!("../../../../packages/core/css/logical/resize.css");
    let css = format!("{PACKAGE_CSS}\n{LOGICAL_SNAP}\n{LOGICAL_RESIZE}");
    for (candidates, declaration) in [
        (
            "snap-x snap-mandatory",
            "scroll-snap-type:inline var(--tw-scroll-snap-strictness)",
        ),
        (
            "snap-y",
            "scroll-snap-type:block var(--tw-scroll-snap-strictness)",
        ),
        ("resize-x", "resize:inline"),
        ("resize-y", "resize:block"),
    ] {
        let (_, result) = compile(&format!("export const x = style('{candidates}');"), &css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.is_empty(), "{result:#?}");
        assert!(result.css.contains(declaration), "{}", result.css);
    }
}
