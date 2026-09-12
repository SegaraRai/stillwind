//! Decoding of arbitrary-value and CSS-variable captures.
//!
//! These helpers are purely textual. They never consult the theme, because a capture
//! that names a variable or an arbitrary value is resolved before any theme lookup
//! happens, and its spelling rules are the same for every namespace.

/// A CSS-variable capture: `(--name)` or `(hint:--name)`.
pub(crate) struct CssVariable<'a> {
    /// Value-type hint written before the colon, when present.
    pub(crate) hint: Option<&'a str>,
    /// The custom-property name, which always starts with `--`.
    pub(crate) name: &'a str,
}

/// Parses a parenthesized custom-property capture.
///
/// Both parentheses and the `--` prefix are required; anything else, including a bare
/// name or an unclosed group, is not a variable capture.
pub(crate) fn parse_css_variable(value: &str) -> Option<CssVariable<'_>> {
    let value = value.strip_prefix('(')?.strip_suffix(')')?;
    let (hint, name) = value
        .split_once(':')
        .map_or((None, value), |(hint, name)| (Some(hint), name));
    name.starts_with("--").then_some(CssVariable { hint, name })
}

/// Decodes an arbitrary value into the text it stands for.
///
/// A backslash escapes the next character, so `\_` is an underscore rather than the
/// space that a bare `_` produces. Every other character is kept as written, which is
/// why this cannot be a general CSS unescaper.
pub(crate) fn decode_arbitrary(value: &str) -> String {
    let mut result = String::new();
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
    result
}

/// Converts an authored alpha modifier into a percentage.
///
/// A value up to and including `1` is a fraction of one and is scaled; a larger value
/// is already a percentage and is only given its unit. A `var()` reference is passed
/// through unchanged because its value is not known here, and anything that is not a
/// number produces [`None`].
pub(crate) fn alpha(value: &str) -> Option<String> {
    if value.starts_with("var(") {
        return Some(value.to_owned());
    }
    let number = value.parse::<f64>().ok()?;
    Some(if number <= 1.0 {
        format!("{}%", number * 100.0)
    } else {
        format!("{number}%")
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
        assert_eq!(alpha(".25"), Some("25%".into()));
        assert_eq!(alpha("75"), Some("75%".into()));
        assert_eq!(alpha("var(--a)"), Some("var(--a)".into()));
        assert_eq!(alpha("red"), None);
    }

    proptest::proptest! {
        #[test]
        fn ordinary_identifiers_survive_decoding(name in "[a-zA-Z0-9-]{0,40}") {
            proptest::prop_assert_eq!(decode_arbitrary(&name), name);
        }
    }
}
