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
            break;
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
