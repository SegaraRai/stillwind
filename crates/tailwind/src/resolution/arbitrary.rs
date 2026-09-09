use std::fmt::Write;

pub(super) struct CssVariable<'a> {
    pub(super) hint: Option<&'a str>,
    pub(super) name: &'a str,
}

pub(super) fn parse_css_variable(value: &str) -> Option<CssVariable<'_>> {
    let value = value.strip_prefix('(')?.strip_suffix(')')?;
    let (hint, name) = value
        .split_once(':')
        .map_or((None, value), |(hint, name)| (Some(hint), name));
    name.starts_with("--").then_some(CssVariable { hint, name })
}

pub(super) fn decode_arbitrary(value: &str) -> String {
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

pub(super) fn alpha(value: &str) -> Option<String> {
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

pub(super) fn important(declaration: &str) -> String {
    declaration
        .split_terminator(';')
        .filter(|declaration| !declaration.is_empty())
        .fold(String::new(), |mut result, declaration| {
            write!(result, "{declaration}!important;").expect("writing to a String cannot fail");
            result
        })
}
