/// Expands composable edge-mask stop utilities. The CSS source owns the
/// utility roots and the affected edges; this helper merely stamps the shared
/// custom-property protocol into each definition. It deliberately enumerates
/// neither colors nor spacing values.
pub(super) fn strip_css_comments(css: &str) -> String {
    let mut output = String::new();
    let mut remaining = css;
    while let Some(start) = remaining.find("/*") {
        output.push_str(&remaining[..start]);
        let Some(end) = remaining[start + 2..].find("*/") else {
            output.push_str(&remaining[start..]);
            return output;
        };
        remaining = &remaining[start + 2 + end + 2..];
    }
    output.push_str(remaining);
    output
}

pub(super) fn extract_named_blocks(css: &str, directive: &str) -> Vec<(String, String)> {
    extract_named_blocks_with_offsets(css, directive)
        .into_iter()
        .map(|(_, header, body)| (header, body))
        .collect()
}

pub(super) fn extract_named_blocks_with_offsets(
    css: &str,
    directive: &str,
) -> Vec<(usize, String, String)> {
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find(directive) {
        let start = cursor + relative;
        let Some(open_relative) = css[start..].find('{') else {
            break;
        };
        let open = start + open_relative;
        let header = css[start + directive.len()..open].trim().to_owned();
        let mut depth = 1_u32;
        let mut close = open;
        for (offset, character) in css[open + 1..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        blocks.push((start, header, css[open + 1..close].trim().to_owned()));
        cursor = close + 1;
    }
    blocks
}

pub(super) fn extract_order_statements(css: &str, directive: &str) -> Vec<(usize, String)> {
    let mut statements = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find(directive) {
        let start = cursor + relative;
        let value_start = start + directive.len();
        let Some(relative_end) = css[value_start..].find(';') else {
            break;
        };
        let end = value_start + relative_end;
        let pattern = css[value_start..end].trim();
        if !pattern.is_empty() && !pattern.chars().any(char::is_whitespace) {
            statements.push((start, pattern.to_owned()));
        }
        cursor = end + 1;
    }
    statements
}

pub(super) fn strip_blocks(css: &str, directive: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find(directive) {
        let start = cursor + relative;
        output.push_str(&css[cursor..start]);
        let Some(open_relative) = find_block_open(&css[start..]) else {
            output.push_str(&css[start..]);
            break;
        };
        let open = start + open_relative;
        let mut depth = 1_u32;
        let mut close = open + 1;
        let body = &css[open + 1..];
        let mut offset = 0;
        while offset < body.len() {
            if body[offset..].starts_with("{modifier}") {
                offset += "{modifier}".len();
                continue;
            }
            let character = body[offset..].chars().next().unwrap();
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
            offset += character.len_utf8();
        }
        if depth != 0 {
            // Preserve malformed input for the parser instead of advancing past EOF.
            output.push_str(&css[start..]);
            return output;
        }
        cursor = close.saturating_add(1);
    }
    if cursor < css.len() {
        output.push_str(&css[cursor..]);
    }
    output
}

pub(super) fn find_block_open(value: &str) -> Option<usize> {
    let mut offset = 0;
    while offset < value.len() {
        if value[offset..].starts_with("{modifier}") {
            offset += "{modifier}".len();
            continue;
        }
        let character = value[offset..].chars().next()?;
        if character == '{' {
            return Some(offset);
        }
        offset += character.len_utf8();
    }
    None
}

pub(super) fn strip_top_level_blocks(css: &str, directive: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    let mut depth = 0_u32;
    while cursor < css.len() {
        if depth == 0 && css[cursor..].starts_with(directive) {
            let Some(open_rel) = css[cursor..].find('{') else {
                break;
            };
            let open = cursor + open_rel;
            let mut inner = 1_u32;
            let mut end = open + 1;
            for (offset, ch) in css[open + 1..].char_indices() {
                match ch {
                    '{' => inner += 1,
                    '}' => {
                        inner = inner.saturating_sub(1);
                        if inner == 0 {
                            end = open + 1 + offset + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            cursor = end;
            continue;
        }
        let Some(ch) = css[cursor..].chars().next() else {
            break;
        };
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth = depth.saturating_sub(1);
        }
        output.push(ch);
        cursor += ch.len_utf8();
    }
    output.push_str(&css[cursor..]);
    output
}

pub(super) fn matching_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in value[open..].char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_blocks_retain_source_offsets_and_nested_bodies() {
        let css = "x @utility foo { color:red; &:hover {color:blue;} }";
        let blocks = extract_named_blocks_with_offsets(css, "@utility");
        assert_eq!(
            blocks,
            vec![(2, "foo".into(), "color:red; &:hover {color:blue;}".into())]
        );
        assert_eq!(extract_named_blocks(css, "@missing"), vec![]);
        assert_eq!(
            extract_order_statements("@order a; @order bad name; @order b-*;", "@order"),
            vec![(0, "a".into()), (27, "b-*".into())]
        );
    }

    #[test]
    fn stripping_preserves_surroundings_and_malformed_blocks() {
        assert_eq!(strip_css_comments("a/* one */b/*two*/c"), "abc");
        assert_eq!(strip_css_comments("a/* unclosed"), "a/* unclosed");
        assert_eq!(strip_blocks("a @theme { x {y} } b", "@theme"), "a  b");
        assert_eq!(strip_blocks("a @theme { x", "@theme"), "a @theme { x");
        assert_eq!(find_block_open("x-{modifier} { y }"), Some(13));
        assert_eq!(
            strip_top_level_blocks("@variant a {x} .a { @variant b {y} }", "@variant"),
            " .a { @variant b {y} }"
        );
        assert_eq!(matching_brace("{ '}' {x} }", 0), Some(10));
        assert_eq!(matching_brace("{x", 0), None);
    }
}
