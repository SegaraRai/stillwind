use crate::functions::{find_css_function, matching_css_delimiter, split_css_function_arguments};

pub(crate) fn resolve_theme_references(
    value: &str,
    mut is_defined: impl FnMut(&str) -> bool,
) -> Option<String> {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(start) = find_css_function(value, "--theme", cursor) {
        output.push_str(&value[cursor..start]);
        let open = start + "--theme".len();
        let close = matching_css_delimiter(value, open, '(', ')')?;
        let arguments = &value[open + 1..close];
        let (reference, fallback) = split_css_function_arguments(arguments);
        let reference = reference.trim();
        if !reference.starts_with("--") {
            return None;
        }
        let fallback = fallback
            .map(str::trim)
            .filter(|fallback| !fallback.is_empty());
        if is_defined(reference) {
            output.push_str("var(");
            output.push_str(reference);
            if let Some(fallback) = fallback.filter(|fallback| *fallback != "initial") {
                output.push_str(", ");
                output.push_str(fallback);
            }
            output.push(')');
        } else {
            let fallback = fallback?;
            if fallback == "initial" {
                return None;
            }
            output.push_str(fallback);
        }
        cursor = close + 1;
    }
    output.push_str(&value[cursor..]);
    Some(output)
}
