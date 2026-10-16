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

/// Decodes an arbitrary value into the text it stands for.
///
/// A backslash escapes the next character, so `\_` is an underscore rather than the
/// space that a bare `_` produces. Every other character is kept as written, except
/// that operators in math functions receive the whitespace described in
/// [`space_math_operators`]. This is why this cannot be a general CSS unescaper.
pub(in crate::evaluation) fn decode_arbitrary(value: &str) -> LeanString {
    let mut result = LeanString::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '_' {
            result.push(' ');
        } else {
            result.push(character);
        }
    }
    if result.contains('(') {
        space_math_operators(&result)
    } else {
        result
    }
}

/// CSS math functions whose arguments [`space_math_operators`] formats.
const MATH_FUNCTIONS: [&str; 19] = [
    "calc", "min", "max", "clamp", "mod", "rem", "sin", "cos", "tan", "asin", "acos", "atan",
    "atan2", "pow", "sqrt", "hypot", "log", "exp", "round",
];

/// Surrounds the binary operators of math functions with spaces, as Tailwind does.
///
/// A candidate cannot contain spaces, so authors write `calc(1rem+2px)`. CSS reads
/// `+2px` there as a signed number, which makes the whole declaration invalid. This
/// function returns `calc(1rem + 2px)` instead.
///
/// It formats `+`, `-`, `*`, and `/` directly inside a math function and inside plain
/// parentheses nested in one. It keeps an operator unchanged when it starts an
/// argument, follows another operator, or is the exponent sign of a number such as
/// `1e-3`. It also keeps an operator between two identifiers, such as the hyphen in
/// `a-b`, because it may belong to the name. Arguments of other functions, such as
/// `var(--a-b)`, are not changed.
fn space_math_operators(value: &str) -> LeanString {
    let characters = value.chars().collect::<Vec<_>>();
    let mut result = String::with_capacity(value.len() + 8);
    // One entry per open parenthesis: whether operators directly inside it are math.
    let mut math = Vec::new();
    // Whether the previous character ends a number or a dimension such as `1rem` or `5%`.
    let mut after_number = false;
    for (index, &character) in characters.iter().enumerate() {
        let previous_ends_number = after_number;
        after_number = character.is_ascii_digit()
            || (after_number && (character == '%' || character.is_ascii_alphabetic()));
        match character {
            '(' => {
                let name_start = result
                    .trim_end_matches(|character: char| {
                        character.is_ascii_lowercase() || character.is_ascii_digit()
                    })
                    .len();
                let name = &result[name_start..];
                math.push(
                    MATH_FUNCTIONS.contains(&name)
                        || (name.is_empty() && math.last() == Some(&true)),
                );
                result.push(character);
            }
            ')' => {
                math.pop();
                result.push(character);
            }
            '+' | '-' | '*' | '/' if math.last() == Some(&true) => {
                let mut trimmed = result.trim_end().chars().rev();
                let previous = trimmed.next();
                let before_previous = trimmed.next();
                let next = characters.get(index + 1).copied();
                let is_sign = matches!(previous, None | Some('(' | ',' | '+' | '-' | '*' | '/'))
                    || (matches!(previous, Some('e' | 'E'))
                        && before_previous.is_some_and(|character| character.is_ascii_digit()));
                if is_sign {
                    result.push(character);
                } else if result.ends_with(char::is_whitespace) {
                    result.push(character);
                    if !next.is_some_and(char::is_whitespace) {
                        result.push(' ');
                    }
                } else if previous_ends_number
                    || previous == Some(')')
                    || next.is_some_and(|next| {
                        next.is_ascii_digit() || matches!(next, '(' | '+' | '-' | '*' | '/')
                    })
                {
                    result.push(' ');
                    result.push(character);
                    result.push(' ');
                } else {
                    result.push(character);
                }
            }
            _ => result.push(character),
        }
    }
    result.into()
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
    fn decoding_preserves_escaped_underscores_and_alpha_units() {
        assert_eq!(decode_arbitrary(r"one_two\_three"), "one two_three");
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

    #[test]
    fn decoding_spaces_binary_operators_in_math_functions() {
        for (authored, decoded) in [
            ("calc(1rem+2px)", "calc(1rem + 2px)"),
            ("calc(100%-2*var(--x))", "calc(100% - 2 * var(--x))"),
            ("calc(1rem_+_2px)", "calc(1rem + 2px)"),
            (
                "min(10px,calc(1rem-(2px+3px)))",
                "min(10px,calc(1rem - (2px + 3px)))",
            ),
            ("calc(var(--a-b)*-1)", "calc(var(--a-b) * -1)"),
            ("calc(1e-3*1px)", "calc(1e-3 * 1px)"),
            ("calc(-1px+2px)", "calc(-1px + 2px)"),
            ("translate(1px-2px)", "translate(1px-2px)"),
            ("nth-child(2n+1)", "nth-child(2n+1)"),
        ] {
            assert_eq!(decode_arbitrary(authored), decoded);
        }
    }

    proptest::proptest! {
        #[test]
        fn ordinary_identifiers_survive_decoding(name in "[a-zA-Z0-9-]{0,40}") {
            proptest::prop_assert_eq!(decode_arbitrary(&name), name);
        }
    }
}
