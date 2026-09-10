/// Expands composable edge-mask stop utilities. The CSS source owns the
/// utility roots and the affected edges; this helper merely stamps the shared
/// custom-property protocol into each definition. It deliberately enumerates
/// neither colors nor spacing values.
pub fn strip_css_comments(css: &str) -> String {
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

pub fn extract_named_blocks(css: &str, directive: &str) -> Vec<(String, String)> {
    extract_named_blocks_with_offsets(css, directive)
        .into_iter()
        .map(|(_, header, body)| (header, body))
        .collect()
}

pub fn extract_named_blocks_with_offsets(
    css: &str,
    directive: &str,
) -> Vec<(usize, String, String)> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| rule.body.map(|body| (rule.start, rule.header, body)))
        .collect()
}

pub fn extract_order_statements(css: &str, directive: &str) -> Vec<(usize, String)> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| {
            (rule.body.is_none()
                && !rule.header.is_empty()
                && !rule.header.chars().any(char::is_whitespace))
            .then_some((rule.start, rule.header))
        })
        .collect()
}

struct DirectiveRule {
    start: usize,
    header: String,
    body: Option<String>,
}

fn directive_rules(css: &str, directive: &str) -> Vec<DirectiveRule> {
    let mut rules = Vec::new();
    scan_directives(
        &mut cssparser::Parser::new(css),
        css,
        directive.trim_start_matches('@'),
        &mut rules,
    );
    rules
}

fn scan_directives(
    parser: &mut cssparser::Parser<'_>,
    css: &str,
    directive: &str,
    rules: &mut Vec<DirectiveRule>,
) {
    use cssparser::{ParseError, Token};
    loop {
        let start = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::AtKeyword(name)) if name.as_ref() == directive => {
                let header_start = parser.position().byte_index();
                loop {
                    match parser.next_including_whitespace_and_comments() {
                        Ok(Token::Semicolon) => {
                            rules.push(DirectiveRule {
                                start,
                                header: css[header_start..parser.position().byte_index() - 1]
                                    .trim()
                                    .into(),
                                body: None,
                            });
                            break;
                        }
                        Ok(Token::CurlyBracketBlock) => {
                            let body_start = parser.position().byte_index();
                            let result = parser.parse_nested_block(|nested| {
                                while nested.next_including_whitespace_and_comments().is_ok() {}
                                Ok::<_, ParseError<()>>(())
                            });
                            if result.is_ok() {
                                let body_end = parser.position().byte_index() - 1;
                                if css.as_bytes().get(body_end) == Some(&b'}') {
                                    rules.push(DirectiveRule {
                                        start,
                                        header: css[header_start..body_start - 1].trim().into(),
                                        body: Some(css[body_start..body_end].trim().into()),
                                    });
                                }
                            }
                            break;
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            }
            Ok(Token::CurlyBracketBlock) => {
                let _ = parser.parse_nested_block(|nested| {
                    scan_directives(nested, css, directive, rules);
                    Ok::<_, ParseError<()>>(())
                });
            }
            Ok(Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock) => {
                let _ = parser.parse_nested_block(|nested| {
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, ParseError<()>>(())
                });
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

pub fn strip_blocks(css: &str, directive: &str) -> String {
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
            let Some(character) = body.get(offset..).and_then(|rest| rest.chars().next()) else {
                break;
            };
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

pub fn find_block_open(value: &str) -> Option<usize> {
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

pub fn matching_brace(value: &str, open: usize) -> Option<usize> {
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

/// Detect EOF inside a block before CSS parsers implicitly close it.
/// Strings, comments and escaped delimiters do not change block depth.
pub fn unterminated_block(css: &str) -> Option<usize> {
    let mut openings = Vec::new();
    let mut quote = None;
    let mut chars = css.char_indices().peekable();
    while let Some((offset, character)) = chars.next() {
        if character == '\\' {
            chars.next();
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '/' if chars.peek().is_some_and(|(_, ch)| *ch == '*') => {
                chars.next();
                while let Some((_, ch)) = chars.next() {
                    if ch == '*' && chars.peek().is_some_and(|(_, ch)| *ch == '/') {
                        chars.next();
                        break;
                    }
                }
            }
            '{' => openings.push(offset),
            '}' => {
                openings.pop();
            }
            _ => {}
        }
    }
    openings.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_ignore_documentation_strings_and_statement_directives() {
        let css = "/* @utility describes a definition. */ .label{content:'@utility fake {x:y}';} @utility-note ignored; @utility real {content:'}'; color:red;}";
        let blocks = extract_named_blocks_with_offsets(css, "@utility");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, css.find("@utility real").unwrap());
        assert_eq!(blocks[0].1, "real");
        assert_eq!(blocks[0].2, "content:'}'; color:red;");
        assert!(
            extract_order_statements("/* @order fake; */ @order real;", "@order")
                .iter()
                .all(|(_, name)| name == "real")
        );
    }

    #[test]
    fn named_blocks_do_not_slice_unclosed_bodies() {
        assert!(extract_named_blocks("@utility x {", "@utility").is_empty());
        assert_eq!(
            extract_named_blocks("@utility ok {color:red;} @utility missing {", "@utility"),
            vec![("ok".into(), "color:red;".into())]
        );
    }

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
        assert_eq!(matching_brace("{ '}' {x} }", 0), Some(10));
        assert_eq!(matching_brace("{x", 0), None);
    }
}
