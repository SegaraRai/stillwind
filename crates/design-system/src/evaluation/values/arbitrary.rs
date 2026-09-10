pub(crate) struct CssVariable<'a> {
    pub(crate) hint: Option<&'a str>,
    pub(crate) name: &'a str,
}

pub(crate) fn parse_css_variable(value: &str) -> Option<CssVariable<'_>> {
    let value = value.strip_prefix('(')?.strip_suffix(')')?;
    let (hint, name) = value
        .split_once(':')
        .map_or((None, value), |(hint, name)| (Some(hint), name));
    name.starts_with("--").then_some(CssVariable { hint, name })
}

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
