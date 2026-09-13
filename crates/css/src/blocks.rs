//! Directive-block scanning for Stillwind's at-rule dialect.
//!
//! These helpers read `@directive header { body }` and `@directive name;` forms out of
//! raw CSS text. Extraction uses cssparser tokens; stripping and brace helpers use
//! simpler textual scans with different handling of strings, escapes and comments. They
//! operate without understanding the declarations they carry: callers decide what a
//! body means.
//!
//! Two behaviours matter for callers. Malformed input is never half-consumed: an
//! unclosed block is skipped (or, when stripping, preserved from the directive onward)
//! so the CSS parser can still report it. The textual stripping helpers specially skip
//! `{modifier}` markers; that exception is not shared by the token scanner.

use cssparser::{ParseError, Parser, Token};

/// Removes complete block comments and preserves an unterminated comment tail.
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

/// Extracts `(header, body)` for every `directive` block, in source order.
///
/// Header text is trimmed and excludes the braces; body text is trimmed. Comment text,
/// strings and other directives are not mistaken for blocks, and a block whose body is
/// unterminated is skipped entirely.
pub fn extract_named_blocks(css: &str, directive: &str) -> Vec<(String, String)> {
    extract_named_blocks_with_offsets(css, directive)
        .into_iter()
        .map(|(_, header, body)| (header, body))
        .collect()
}

/// Like [`extract_named_blocks`], but also returns the byte offset of the directive's
/// `@`.
///
/// Callers use the offset to attribute a block to a position in the original source;
/// offsets index `css` itself, so callers that stripped comments must map them back
/// before reporting.
pub fn extract_named_blocks_with_offsets(
    css: &str,
    directive: &str,
) -> Vec<(usize, String, String)> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| rule.body.map(|body| (rule.start, rule.header, body)))
        .collect()
}

/// Extracts `(offset, name)` for `directive name;` statements.
///
/// A statement must have no body, a non-empty name, and no whitespace in that name, so
/// `@order a b;` is ignored rather than reported as a name.
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

/// Extracts `(offset, header)` from bodyless directives without restricting the header.
/// Nested functions and quoted text are kept intact by the directive token scanner.
pub fn extract_named_statements(css: &str, directive: &str) -> Vec<(usize, String)> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| rule.body.is_none().then_some((rule.start, rule.header)))
        .collect()
}

/// Reads semantic registration requirements from a utility definition or fragment.
/// Quoted/comment text is ignored. Unknown property names are validated by the
/// consumer against its registry; malformed directive arguments reject the input.
pub fn property_requirement_ranges(
    css: &str,
) -> Option<Vec<(std::ops::Range<usize>, Vec<String>)>> {
    if !css.contains("@stillwind-require-properties") {
        return Some(Vec::new());
    }
    directive_rules(css, "stillwind-require-properties")
        .into_iter()
        .map(|rule| {
            if rule.body.is_some() {
                return None;
            }
            let names = rule
                .header
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if names.is_empty()
                || names.iter().any(|name| {
                    !name.starts_with("--")
                        || name.len() == 2
                        || !name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                })
            {
                return None;
            }
            let end = rule.start + css[rule.start..].find(';')? + 1;
            Some((rule.start..end, names))
        })
        .collect()
}

/// Lowers registration requirements before declarations are serialized as CSS.
pub fn take_property_requirements(css: &str) -> Option<(String, Vec<String>)> {
    let requests = property_requirement_ranges(css)?;
    let mut output = String::new();
    let mut properties = Vec::new();
    let mut cursor = 0;
    for (range, names) in requests {
        output.push_str(&css[cursor..range.start]);
        cursor = range.end;
        properties.extend(names);
    }
    output.push_str(&css[cursor..]);
    Some((output, properties))
}

/// One directive occurrence found by [`scan_directives`].
struct DirectiveRule {
    /// UTF-8 byte offset of the directive's `@` in the scanned input.
    start: usize,
    /// Trimmed prelude without the directive name, braces, or statement semicolon;
    /// internal whitespace is retained.
    header: String,
    /// Trimmed text inside the outer braces; None represents a bodyless statement.
    body: Option<String>,
}

/// Collects every occurrence of one directive name.
///
/// The name is compared without its leading `@`.
fn directive_rules(css: &str, directive: &str) -> Vec<DirectiveRule> {
    let mut rules = Vec::new();
    scan_directives(
        &mut Parser::new(css),
        css,
        directive.trim_start_matches('@'),
        &mut rules,
    );
    rules
}

/// Walks a token stream, recording matches and recursing into rule blocks.
///
/// Nested curly blocks are descended into, because a directive may appear inside
/// `@layer`; function, parenthesis and square-bracket blocks are consumed but not
/// searched, so directive-shaped text inside `url()` or a string value is never treated
/// as a directive.
fn scan_directives(
    parser: &mut Parser<'_>,
    css: &str,
    directive: &str,
    rules: &mut Vec<DirectiveRule>,
) {
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

/// Removes every `directive { ... }` block, including nested braces.
///
/// `{modifier}` is skipped as literal text rather than counted as an opening brace. An
/// unterminated body is preserved from the directive onward. This textual helper does
/// not distinguish strings or comments from directive text and braces.
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

/// Byte offset of the first `{` that is not the start of a `{modifier}` marker.
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

/// Returns the offset of the `}` matching the `{` at `open`.
///
/// Quoted strings and escapes are respected; `None` means the brace is never closed or
/// a `}` appears first.
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

/// Detect EOF inside a block before CSS parsers implicitly close it. Strings, comments
/// and escaped delimiters do not change block depth.
///
/// Returns the offset of the outermost `{` that is still open at end of input, or
/// `None` when every block was closed.
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

#[cfg(test)]
mod requirement_tests {
    use super::take_property_requirements;
    #[test]
    fn requirements_are_lexical_and_reject_non_names() {
        let css = r".x{content:'@stillwind-require-properties --fake;';@stillwind-require-properties --real;}";
        let (lowered, names) = take_property_requirements(css).unwrap();
        assert_eq!(names, ["--real"]);
        assert!(lowered.contains("'@stillwind-require-properties --fake;'"));
        assert!(take_property_requirements("@stillwind-require-properties red;").is_none());
        assert!(take_property_requirements("@stillwind-require-properties --x{}").is_none());
    }
}
