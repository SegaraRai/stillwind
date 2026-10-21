//! Directive-block scanning for Stillwind's at-rule dialect.
//!
//! These helpers read `@directive header { body }` and `@directive name;` forms out of
//! raw CSS text. Extraction and comment stripping use cssparser tokens; brace helpers
//! use simpler textual scans with different handling of strings, escapes and comments.
//! They operate without understanding the declarations they carry: callers decide what
//! a body means.
//!
//! Malformed input is never half-consumed: an unclosed block is skipped, and an
//! unterminated comment is kept, so the CSS parser can still report it.

use std::ops::Range;

use cssparser::{ParseError, Parser, SourcePosition, Token};
use lean_string::{LeanStr, LeanString};

/// Removes complete comments and keeps all other text unchanged.
///
/// Strings and unquoted URLs are tokens, so `/*` inside them is not a comment. An
/// unterminated comment is kept.
pub fn strip_css_comments(css: &str) -> LeanString {
    let mut output = LeanString::with_capacity(css.len());
    copy_without_comments(&mut Parser::new(css), &mut output);
    output
}

/// Appends the remaining input of `parser` to `output` without complete comments,
/// including the contents of nested blocks.
fn copy_without_comments(parser: &mut Parser<'_>, output: &mut LeanString) {
    loop {
        let start = parser.position();
        let block = match parser.next_including_whitespace_and_comments() {
            Err(_) => return,
            Ok(Token::Comment(_)) => {
                let comment = parser.slice_from(start);
                // `/*/` ends with `*/` but is unterminated.
                if comment.len() < 4 || !comment.ends_with("*/") {
                    output.push_str(comment);
                }
                continue;
            }
            Ok(
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock,
            ) => true,
            Ok(_) => false,
        };
        output.push_str(parser.slice_from(start));
        if block {
            let mut contents_end: Option<SourcePosition> = None;
            let _ = parser.parse_nested_block(|nested| {
                copy_without_comments(nested, output);
                contents_end = Some(nested.position());
                Ok::<_, ParseError<()>>(())
            });
            if let Some(contents_end) = contents_end {
                // The closing delimiter, or nothing for a block closed by the end of input.
                output.push_str(parser.slice(contents_end..parser.position()));
            }
        }
    }
}

/// Extracts `(offset, header, body)` for every `directive` block, in source order.
/// `offset` is the byte offset of the directive's `@`.
///
/// Header text is trimmed and excludes the braces; body text is trimmed. Comment text,
/// strings and other directives are not mistaken for blocks, and a block whose body is
/// unterminated is skipped entirely.
///
/// Callers use the offset to attribute a block to a position in the original source;
/// offsets index `css` itself, so callers that stripped comments must map them back
/// before reporting.
pub fn extract_named_blocks(css: &str, directive: &str) -> Vec<(usize, LeanStr, LeanStr)> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| rule.body.map(|body| (rule.start, rule.header, body)))
        .collect()
}

/// Extracts `(offset, name)` for `directive name;` statements.
///
/// A statement must have no body, a non-empty name, and no whitespace in that name, so
/// `@order a b;` is ignored rather than reported as a name.
pub fn extract_order_statements(css: &str, directive: &str) -> Vec<(usize, LeanStr)> {
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
pub fn extract_named_statements(css: &str, directive: &str) -> Vec<(usize, LeanStr)> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| rule.body.is_none().then_some((rule.start, rule.header)))
        .collect()
}

/// A malformed property requirement and its byte offset in the supplied CSS body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid property requirement at byte {offset}")]
pub struct PropertyRequirementError {
    /// Byte offset of the directive within the supplied CSS body.
    pub offset: usize,
}

/// One registration requirement directive and its named properties.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PropertyRequirement {
    /// Byte range of the full directive in its CSS body.
    range: Range<usize>,
    /// Required custom-property names in authored order.
    names: Vec<LeanStr>,
}

/// Reads semantic registration requirements from a utility definition or fragment.
/// Quoted/comment text is ignored. Unknown property names are validated by the
/// consumer against its registry; malformed directive arguments reject the input.
///
/// # Errors
///
/// Returns the byte offset of the first malformed directive.
fn property_requirement_ranges(
    css: &str,
) -> Result<Vec<PropertyRequirement>, PropertyRequirementError> {
    if !css.contains("@stillwind-require-properties") {
        return Ok(Vec::new());
    }
    directive_rules(css, "stillwind-require-properties")
        .into_iter()
        .map(|rule| {
            if rule.body.is_some() {
                return Err(PropertyRequirementError { offset: rule.start });
            }
            let names = rule
                .header
                .split_whitespace()
                .map(LeanStr::from)
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
                return Err(PropertyRequirementError { offset: rule.start });
            }
            let end = rule.start
                + css[rule.start..]
                    .find(';')
                    .ok_or(PropertyRequirementError { offset: rule.start })?
                + 1;
            Ok(PropertyRequirement {
                range: rule.start..end,
                names,
            })
        })
        .collect()
}

/// Lowers registration requirements before declarations are serialized as CSS.
///
/// # Errors
///
/// Returns the byte offset of the first malformed directive.
pub fn take_property_requirements(
    css: &str,
) -> Result<(LeanString, Vec<LeanStr>), PropertyRequirementError> {
    let requests = property_requirement_ranges(css)?;
    let mut output = LeanString::new();
    let mut properties = Vec::new();
    let mut cursor = 0;
    for PropertyRequirement { range, names } in requests {
        output.push_str(&css[cursor..range.start]);
        cursor = range.end;
        properties.extend(names);
    }
    output.push_str(&css[cursor..]);
    Ok((output, properties))
}

/// One directive occurrence found by [`scan_directives`].
struct DirectiveRule {
    /// UTF-8 byte offset of the directive's `@` in the scanned input.
    start: usize,
    /// Trimmed prelude without the directive name, braces, or statement semicolon;
    /// internal whitespace is retained.
    header: LeanStr,
    /// Trimmed text inside the outer braces; None represents a bodyless statement.
    body: Option<LeanStr>,
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

/// Reason a byte-offset delimiter scan cannot return a closing offset.
///
/// The brace and delimiter scanners share it, so a caller can tell an unusable
/// starting offset apart from input that does not close the delimiter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScanError {
    /// The requested offset is past the end of the input or inside a multi-byte scalar.
    #[error("scan offset {offset} is not a character boundary within {length} bytes")]
    InvalidOffset {
        /// Requested byte offset.
        offset: usize,
        /// Length in bytes of the scanned input.
        length: usize,
    },
    /// The delimiter is never closed, or a closer appears before its opener.
    #[error("delimiter has no matching closer")]
    Unclosed,
}

/// Returns the offset of the `}` matching the `{` at `open`.
///
/// Quoted strings and escapes are respected, so braces inside them are literal text.
///
/// # Errors
///
/// Returns [`ScanError::InvalidOffset`] when `open` is past the end of `value` or
/// falls inside a multi-byte scalar, and [`ScanError::Unclosed`] when the brace is
/// never closed or a `}` appears first. Offsets from `char_indices` over this same
/// string satisfy the character-boundary requirement.
pub fn matching_brace(value: &str, open: usize) -> Result<usize, ScanError> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    let rest = value.get(open..).ok_or(ScanError::InvalidOffset {
        offset: open,
        length: value.len(),
    })?;
    for (offset, character) in rest.char_indices() {
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
                depth = depth.checked_sub(1).ok_or(ScanError::Unclosed)?;
                if depth == 0 {
                    return Ok(open + offset);
                }
            }
            _ => {}
        }
    }
    Err(ScanError::Unclosed)
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
        let blocks = extract_named_blocks(css, "@utility");
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
        assert_eq!(
            extract_named_blocks("@utility x {", "@utility"),
            [] as [(usize, LeanStr, LeanStr); 0]
        );
        assert_eq!(
            extract_named_blocks("@utility ok {color:red;} @utility missing {", "@utility"),
            vec![(0, "ok".into(), "color:red;".into())]
        );
    }

    #[test]
    fn named_blocks_retain_source_offsets_and_nested_bodies() {
        let css = "x @utility foo { color:red; &:hover {color:blue;} }";
        let blocks = extract_named_blocks(css, "@utility");
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
        assert_eq!(strip_css_comments("a/*/"), "a/*/");
        assert_eq!(
            strip_css_comments(r#"x{content:"/*";/* c */b:url(/*x*/) fn(1/**/2) [a/**/]}"#),
            r#"x{content:"/*";b:url(/*x*/) fn(12) [a]}"#
        );
        assert_eq!(strip_css_comments("x{a:(b/**/"), "x{a:(b");
        assert_eq!(matching_brace("{ '}' {x} }", 0), Ok(10));
        assert_eq!(matching_brace("{x", 0), Err(ScanError::Unclosed));
        assert_eq!(
            matching_brace("{x", 3),
            Err(ScanError::InvalidOffset {
                offset: 3,
                length: 2
            })
        );
        assert_eq!(
            matching_brace("é{x}", 1),
            Err(ScanError::InvalidOffset {
                offset: 1,
                length: 5
            })
        );
        assert_eq!(matching_brace("x}", 0), Err(ScanError::Unclosed));
    }

    /// Statements and blocks are recorded in source order with their trimmed header,
    /// optional body, and the offset of the directive's own `@`.
    #[test]
    fn scan_directives_records_statements_and_blocks_in_source_order() {
        let css = "@foo  spaced header; .x{color:red} @foo block header { body content }";
        let mut rules = Vec::new();
        scan_directives(&mut Parser::new(css), css, "foo", &mut rules);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].start, 0);
        assert_eq!(rules[0].header, "spaced header");
        assert_eq!(rules[0].body, None);
        assert_eq!(rules[1].start, css.find("@foo block").unwrap());
        assert_eq!(rules[1].header, "block header");
        assert_eq!(rules[1].body.as_deref(), Some("body content"));
    }

    /// A directive is found inside nested rule blocks, while directive-shaped text in a
    /// function, parenthesis, bracket, or string is consumed without being searched.
    #[test]
    fn scan_directives_descends_into_rule_blocks_only() {
        let css = "@layer x { @foo nested; } .a{background:url(@foo no;);content:'@foo no;';} (@foo no;) [@foo no;]";
        let mut rules = Vec::new();
        scan_directives(&mut Parser::new(css), css, "foo", &mut rules);
        assert_eq!(rules.len(), 1, "{:?}", rules.len());
        assert_eq!(rules[0].header, "nested");
        assert_eq!(rules[0].body, None);
    }

    /// An unterminated statement or block is dropped, and the directive name is matched
    /// exactly.
    #[test]
    fn scan_directives_skips_unterminated_directives_and_other_names() {
        for css in ["@foo unterminated", "@foo open { body"] {
            let mut rules = Vec::new();
            scan_directives(&mut Parser::new(css), css, "foo", &mut rules);
            assert!(rules.is_empty(), "{css:?} must not record a rule");
        }
        // A statement with an empty prelude is still a statement.
        let mut rules = Vec::new();
        scan_directives(&mut Parser::new("@foo ;"), "@foo ;", "foo", &mut rules);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].header, "");
        assert_eq!(rules[0].body, None);
        let css = "@foobar x; @Foo x; @foo y;";
        let mut rules = Vec::new();
        scan_directives(&mut Parser::new(css), css, "foo", &mut rules);
        assert_eq!(rules.len(), 1, "{:?}", rules.len());
        assert_eq!(rules[0].header, "y");
    }

    #[test]
    fn requirements_are_lexical_and_reject_non_names() {
        let css = r".x{content:'@stillwind-require-properties --fake;';@stillwind-require-properties --real;}";
        let (lowered, names) = take_property_requirements(css).unwrap();
        assert_eq!(names, ["--real"]);
        assert!(lowered.contains("'@stillwind-require-properties --fake;'"));
        assert!(take_property_requirements("@stillwind-require-properties red;").is_err());
        assert!(take_property_requirements("@stillwind-require-properties --x{}").is_err());
    }
}
