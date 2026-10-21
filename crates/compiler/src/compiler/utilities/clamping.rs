//! Literal values that browsers clamp into the range of their property or filter
//! function.
//!
//! The ranges come from specification prose, not from the property grammars: CSS
//! Color 4 clamps an `<opacity-value>` to `0` through `1`, and Filter Effects 1 clamps
//! the amount of `opacity()`, `grayscale()`, `sepia()`, and `invert()` to at most `1`
//! and forbids negative amounts. The grammars accept every number, so the curated
//! tables below record these ranges.

use lean_string::ToLeanStr;

use stillwind_css::{declaration_ranges, evaluate_numeric, find_css_functions};
use stillwind_diagnostics::LintDiagnostic;

/// Properties whose value browsers clamp to `0` through `1`, or `0%` through `100%`.
const CLAMPED_PROPERTIES: &[&str] = &[
    "opacity",
    "fill-opacity",
    "stroke-opacity",
    "flood-opacity",
    "stop-opacity",
];

/// Filter functions whose amount browsers clamp to at most `1`, or `100%`. A negative
/// amount makes the filter invalid instead.
const CLAMPED_FILTERS: &[&str] = &["opacity", "grayscale", "sepia", "invert"];

/// Returns the finding for the first literal value in `declarations` that lies outside
/// the range of its property or filter function, or `None` when every value is in
/// range or is not a number or percentage.
///
/// `declarations` are serialized declaration lists in emission order. A value above or
/// below the range of a property, and a filter amount above `1`, select
/// `clamped-value`. A negative filter amount selects `incompatible-property-value` for
/// `filter`, or for `backdrop-filter` when the declaring property's name contains
/// `backdrop`, because Tailwind's filter utilities pass amounts through custom
/// properties with such names. `utility` is the authored utility that the
/// `incompatible-property-value` message names.
pub(in crate::compiler) fn clamped_value<'a>(
    declarations: impl IntoIterator<Item = &'a str>,
    utility: &str,
) -> Option<LintDiagnostic> {
    for list in declarations {
        for range in declaration_ranges(list) {
            let Some((property, value)) = list[range].split_once(':') else {
                continue;
            };
            let (property, value) = (property.trim(), value.trim());
            if let Some(diagnostic) = clamped_property(property, value) {
                return Some(diagnostic);
            }
            if let Some(diagnostic) = clamped_filter(property, value, utility) {
                return Some(diagnostic);
            }
        }
    }
    None
}

fn clamped_property(property: &str, value: &str) -> Option<LintDiagnostic> {
    if !CLAMPED_PROPERTIES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(property))
    {
        return None;
    }
    let value = value
        .strip_suffix("!important")
        .map_or(value, str::trim_end);
    let (number, unit) = evaluate_numeric(value)?;
    let (low, high) = match unit.as_str() {
        "" => (0.0, 1.0),
        "%" => (0.0, 100.0),
        _ => return None,
    };
    let clamped = number.clamp(low, high);
    #[allow(clippy::float_cmp)] // `clamp` returns `number` itself when it is in range.
    let unchanged = clamped == number;
    (!unchanged).then(|| LintDiagnostic::ClampedValue {
        value: format_args!("{property}: {value}").to_lean_str(),
        clamped: format_args!("{property}: {clamped}{unit}").to_lean_str(),
    })
}

fn clamped_filter(property: &str, value: &str, utility: &str) -> Option<LintDiagnostic> {
    for function in find_css_functions(value, CLAMPED_FILTERS, 0) {
        let end = function.end?;
        let name = CLAMPED_FILTERS[function.name_index];
        let Some((amount, unit)) = evaluate_numeric(&value[function.arguments.clone()]) else {
            continue;
        };
        let high = match unit.as_str() {
            "" => 1.0,
            "%" => 100.0,
            _ => continue,
        };
        let written = &value[function.start..end];
        if amount < 0.0 {
            let filter = if property.contains("backdrop") {
                "backdrop-filter"
            } else {
                "filter"
            };
            return Some(LintDiagnostic::IncompatiblePropertyValue {
                utility: utility.to_lean_str(),
                property: filter.into(),
                value: written.to_lean_str(),
                alternatives: Vec::new(),
            });
        }
        if amount > high {
            return Some(LintDiagnostic::ClampedValue {
                value: written.to_lean_str(),
                clamped: format_args!("{name}({high}{unit})").to_lean_str(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::*;

    fn message(declarations: &[&str]) -> Option<String> {
        clamped_value(declarations.iter().copied(), "u")
            .map(|diagnostic| format!("{} {}", diagnostic.code(), diagnostic.to_lean_string()))
    }

    #[test]
    fn values_outside_property_and_filter_ranges_are_reported() {
        for (declarations, expected) in [
            (
                &["opacity:calc(150 * 1%);"][..],
                "clamped-value browsers clamp `opacity: calc(150 * 1%)` to `opacity: 100%`",
            ),
            (
                &["opacity:-0.5;"],
                "clamped-value browsers clamp `opacity: -0.5` to `opacity: 0`",
            ),
            (
                &["color:red;", "stroke-opacity:1.5 !important;"],
                "clamped-value browsers clamp `stroke-opacity: 1.5` to `stroke-opacity: 1`",
            ),
            (
                &["--tw-grayscale:grayscale(150%);filter:var(--tw-grayscale,);"],
                "clamped-value browsers clamp `grayscale(150%)` to `grayscale(100%)`",
            ),
            (
                &["--tw-backdrop-sepia:sepia(2);"],
                "clamped-value browsers clamp `sepia(2)` to `sepia(1)`",
            ),
            (
                &["--tw-invert:invert(-50%);"],
                "incompatible-property-value `filter` does not accept `invert(-50%)`",
            ),
            (
                &["--tw-backdrop-opacity:opacity(-1);"],
                "incompatible-property-value `backdrop-filter` does not accept `opacity(-1)`",
            ),
        ] {
            assert_eq!(message(declarations).as_deref(), Some(expected));
        }
    }

    #[test]
    fn values_in_range_or_without_a_literal_amount_are_not_reported() {
        for declarations in [
            &["opacity:100%;", "opacity:0;", "opacity:var(--x);"][..],
            &[
                "--tw-grayscale:grayscale(100%);",
                "--tw-sepia:sepia(var(--a));",
            ],
            &["width:150%;", "--tw-brightness:brightness(150%);"],
            &["opacity:1px;"],
        ] {
            assert_eq!(message(declarations), None, "{declarations:?}");
        }
    }
}
