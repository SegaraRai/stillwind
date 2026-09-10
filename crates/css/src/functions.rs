pub fn find_css_function(value: &str, name: &str, from: usize) -> Option<usize> {
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

pub fn matching_css_delimiter(
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

pub fn split_css_function_arguments(arguments: &str) -> (&str, Option<&str>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_quoted_and_commented_functions_and_handles_unclosed_input() {
        let value = "'--theme(fake)' /* --theme(no) */ --theme(--yes)";
        assert_eq!(
            find_css_function(value, "--theme", 0),
            value.rfind("--theme")
        );
        assert_eq!(find_css_function("/* never closed", "--theme", 0), None);
        assert_eq!(
            matching_css_delimiter("(a(')') /* ) */)", 0, '(', ')'),
            Some(15)
        );
        assert_eq!(matching_css_delimiter("(a", 0, '(', ')'), None);
    }

    #[test]
    fn splits_only_the_first_unquoted_top_level_comma() {
        assert_eq!(
            split_css_function_arguments("var(--a, red), rgb(1, 2, 3)"),
            ("var(--a, red)", Some(" rgb(1, 2, 3)"))
        );
        assert_eq!(split_css_function_arguments("'a,b'"), ("'a,b'", None));
        assert_eq!(
            split_css_function_arguments("[a,b],x,y"),
            ("[a,b]", Some("x,y"))
        );
        assert_eq!(split_css_function_arguments(""), ("", None));
    }
}
