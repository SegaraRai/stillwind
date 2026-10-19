//! Decoding of arbitrary-value and CSS-variable captures.
//!
//! These helpers are purely textual. They never consult the theme, because a capture
//! that names a variable or an arbitrary value is resolved before any theme lookup
//! happens, and its spelling rules are the same for every namespace.

use lean_string::{LeanString, ToLeanString};

/// A CSS-variable capture: `(--name)` or `(hint:--name)`.
pub(in crate::evaluation) struct CssVariable<'a> {
    /// Value-type hint written before the colon, when present.
    pub(in crate::evaluation) hint: Option<&'a str>,
    /// The custom-property name, which always starts with `--`.
    pub(in crate::evaluation) name: &'a str,
}

/// Parses a parenthesized custom-property capture.
///
/// Both parentheses and the `--` prefix are required; anything else, including a bare
/// name or an unclosed group, is not a variable capture.
pub(in crate::evaluation) fn parse_css_variable(value: &str) -> Option<CssVariable<'_>> {
    let value = value.strip_prefix('(')?.strip_suffix(')')?;
    let (hint, name) = value
        .split_once(':')
        .map_or((None, value), |(hint, name)| (Some(hint), name));
    name.starts_with("--").then_some(CssVariable { hint, name })
}

/// An alpha modifier converted into a percentage.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::evaluation) struct Alpha {
    /// The percentage, or the `var()` reference as written.
    pub(in crate::evaluation) value: LeanString,
    /// The percentage lies outside `0%` to `100%`, which `color-mix()` rejects.
    pub(in crate::evaluation) out_of_range: bool,
}

/// Converts an authored alpha modifier into a percentage.
///
/// A value up to and including `1` is a fraction of one and is scaled; a larger value
/// is already a percentage and is only given its unit. A percentage is kept as written.
/// A `var()` reference is passed through unchanged because its value is not known here.
/// Anything else produces [`None`]. A percentage outside `0%` to `100%` is kept as
/// Tailwind keeps it and is marked as out of range.
pub(in crate::evaluation) fn alpha(value: &str) -> Option<Alpha> {
    if value.starts_with("var(") {
        return Some(Alpha {
            value: value.to_lean_string(),
            out_of_range: false,
        });
    }
    let (value, percentage) = if let Some(percentage) = value.strip_suffix('%') {
        (value.to_lean_string(), percentage.parse::<f64>().ok()?)
    } else {
        let number = value.parse::<f64>().ok()?;
        let percentage = if number <= 1.0 {
            number * 100.0
        } else {
            number
        };
        (format_args!("{percentage}%").to_lean_string(), percentage)
    };
    Some(Alpha {
        value,
        out_of_range: !(0.0..=100.0).contains(&percentage),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_require_wrappers_and_dashed_names() {
        let variable = parse_css_variable("(color:--accent)").unwrap();
        assert_eq!(variable.hint, Some("color"));
        assert_eq!(variable.name, "--accent");
        assert_eq!(parse_css_variable("(--x)").unwrap().hint, None);
        for value in ["--x", "(x)", "(--x"] {
            assert!(parse_css_variable(value).is_none());
        }
    }

    #[test]
    fn alpha_modifiers_become_percentages() {
        let alpha = |value| alpha(value).map(|alpha| (alpha.value, alpha.out_of_range));
        assert_eq!(alpha(".25"), Some(("25%".into(), false)));
        assert_eq!(alpha("75"), Some(("75%".into(), false)));
        assert_eq!(alpha("var(--a)"), Some(("var(--a)".into(), false)));
        assert_eq!(alpha("red"), None);
        assert_eq!(alpha("50%"), Some(("50%".into(), false)));
        assert_eq!(alpha("150"), Some(("150%".into(), true)));
        assert_eq!(alpha("150%"), Some(("150%".into(), true)));
        assert_eq!(alpha("-5%"), Some(("-5%".into(), true)));
    }
}
