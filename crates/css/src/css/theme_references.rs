use super::BTreeSet;

pub(super) fn custom_property_references(css: &str) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("--") {
        let start = cursor + relative;
        let end = css[start..]
            .char_indices()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .last()
            .map_or(start + 2, |(offset, character)| {
                start + offset + character.len_utf8()
            });
        if end > start + 2 {
            references.insert(css[start..end].to_owned());
        }
        cursor = end.max(start + 2);
    }
    references
}

pub(super) fn resolve_theme_references(
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

pub(super) fn find_css_function(value: &str, name: &str, from: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    let needle = format!("{name}(");
    let mut quote = None;
    let mut escaped = false;
    let mut index = from;
    while index < bytes.len() {
        let character = bytes[index] as char;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = value[index + 2..].find("*/")?;
                index += end + 4;
                continue;
            }
            _ if bytes[index] == b'-' && value[index..].starts_with(&needle) => {
                return Some(index);
            }
            _ => {}
        }
        index += 1;
    }
    None
}

pub(super) fn matching_css_delimiter(
    value: &str,
    open: usize,
    opener: char,
    closer: char,
) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    let bytes = value.as_bytes();
    let mut index = open;
    while index < bytes.len() {
        let character = bytes[index] as char;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = value[index + 2..].find("*/")?;
                index += end + 4;
                continue;
            }
            _ if character == opener => depth += 1,
            _ if character == closer => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

pub(super) fn split_css_function_arguments(arguments: &str) -> (&str, Option<&str>) {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in arguments.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' | '[' | '{' => stack.push(character),
            ')' | ']' | '}' => {
                stack.pop();
            }
            ',' if stack.is_empty() => {
                return (&arguments[..index], Some(&arguments[index + 1..]));
            }
            _ => {}
        }
    }
    (arguments, None)
}
