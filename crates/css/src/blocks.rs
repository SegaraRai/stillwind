//! At-rule extraction, comment ranges, and comment removal for CSS text.
//!
//! These helpers read `@directive header { body }` and `@directive name;` forms out of
//! raw CSS text. Extraction, comment stripping, and block boundaries use cssparser
//! tokens. They operate without understanding the declarations they carry: callers
//! decide what a body means.
//!
//! Malformed input is never half-consumed: an unclosed block is skipped, and an
//! unterminated comment is kept, so the CSS parser can still report it.

use std::ops::Range;

use cssparser::{ParseError, Parser, SourcePosition, Token};
use lean_string::{LeanStr, LeanString};

use crate::parser::{closer, consume, consume_css_block, literal_closed, parse_css_block};

/// Returns CSS comment bodies in authored order, excluding their delimiters.
/// An unterminated comment extends to EOF. Strings and URL tokens contain no comments.
pub fn css_comment_ranges(css: &str) -> Vec<Range<usize>> {
    fn collect(parser: &mut Parser<'_>, css: &str, comments: &mut Vec<Range<usize>>) {
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments() else {
                return;
            };
            let comment = matches!(token, Token::Comment(_));
            let block = closer(token).is_some();
            if comment {
                let end = parser.position().byte_index();
                let closed = literal_closed(&css[start..end], "*/");
                comments.push(start + 2..end - if closed { 2 } else { 0 });
            }
            if block {
                let _ = parse_css_block(parser, |inner| collect(inner, css, comments));
            }
        }
    }
    let mut comments = Vec::new();
    collect(&mut Parser::new(css), css, &mut comments);
    comments
}

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

/// One `directive header { body }` block and its byte ranges in the scanned input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectiveBlock {
    /// Trimmed prelude without the directive name and braces.
    pub header: LeanStr,
    /// Byte range from the directive's `@` through the closing brace.
    pub range: Range<usize>,
    /// Byte range inside the outer braces, untrimmed.
    pub body: Range<usize>,
}

/// Extracts every complete `directive` block with its byte ranges, in source order.
///
/// Blocks are found as [`extract_named_blocks`] finds them, so a block nested in
/// another block of the same directive is also returned. A bodyless statement is not
/// a block.
pub fn extract_directive_blocks(css: &str, directive: &str) -> Vec<DirectiveBlock> {
    directive_rules(css, directive)
        .into_iter()
        .filter_map(|rule| {
            Some(DirectiveBlock {
                header: rule.header,
                range: rule.range,
                body: rule.body_range?,
            })
        })
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

/// One directive occurrence found by [`scan_directives`].
struct DirectiveRule {
    /// UTF-8 byte offset of the directive's `@` in the scanned input.
    start: usize,
    /// Trimmed prelude without the directive name, braces, or statement semicolon;
    /// internal whitespace is retained.
    header: LeanStr,
    /// Trimmed text inside the outer braces; None represents a bodyless statement.
    body: Option<LeanStr>,
    /// Byte range from the `@` through the closing brace or semicolon.
    range: Range<usize>,
    /// Byte range inside the outer braces, untrimmed; None for a statement.
    body_range: Option<Range<usize>>,
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
                                range: start..parser.position().byte_index(),
                                body_range: None,
                            });
                            break;
                        }
                        Ok(Token::CurlyBracketBlock) => {
                            let body_start = parser.position().byte_index();
                            let result = consume_css_block(parser);
                            if let Ok(block) = result
                                && let Some(body_end) = block.close
                            {
                                rules.push(DirectiveRule {
                                    start,
                                    header: css[header_start..body_start - 1].trim().into(),
                                    body: Some(css[body_start..body_end].trim().into()),
                                    range: start..body_end + 1,
                                    body_range: Some(body_start..body_end),
                                });
                            }
                            break;
                        }
                        Ok(_) => {}
                        Err(_) => {
                            break;
                        }
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
            Err(_) => {
                break;
            }
        }
    }
}

/// Detect EOF inside a block before CSS parsers implicitly close it. Strings, comments
/// and escaped delimiters do not change block depth.
///
/// Returns the offset of the outermost `{` that is still open at end of input, or
/// `None` when every block was closed.
pub fn unterminated_block(css: &str) -> Option<usize> {
    unclosed_curly(&mut Parser::new(css))
}

fn unclosed_curly(parser: &mut Parser<'_>) -> Option<usize> {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return None;
        };
        let curly = matches!(token, Token::CurlyBracketBlock);
        if closer(token).is_some() {
            let (child, block) = parse_css_block(parser, |inner| {
                let child = unclosed_curly(inner);
                consume(inner);
                child
            })
            .ok()?;
            if curly && block.close.is_none() {
                return Some(start);
            }
            if child.is_some() {
                return child;
            }
        }
    }
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
}
