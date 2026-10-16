//! Rule structure of authored stylesheets.
//!
//! [`parse_structure`] tokenizes CSS with cssparser and returns the style rules,
//! at-rules, declarations, and `@apply` statements of each block, with byte ranges into
//! the input. Comments and strings never start a node, so directive-shaped text inside
//! them is not a directive. The compiler expands `@apply` and `@variant` through this
//! model, and stylesheet lint reports at its ranges, so both agree on which text is a
//! directive.
//!
//! The parser accepts malformed input. An unclosed block ends at the end of the input,
//! and text that is neither a rule nor a declaration contributes no node. Callers that
//! need validation parse the stylesheet separately.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanStr;

/// One rule, at-rule, `@apply` statement, or declaration with its byte range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssNode {
    /// Range from the first token of the node through its closing brace or terminating
    /// semicolon. A node that the input leaves unterminated ends at its last token.
    pub range: Range<usize>,
    /// Form of the node and the ranges of its parts.
    pub kind: CssNodeKind,
}

impl CssNode {
    /// Returns the nodes inside the block of a rule or at-rule, or an empty slice for a
    /// statement or declaration.
    pub fn children(&self) -> &[Self] {
        match &self.kind {
            CssNodeKind::StyleRule { block, .. }
            | CssNodeKind::AtRule {
                block: Some(block), ..
            } => &block.children,
            _ => &[],
        }
    }
}

/// The form of a [`CssNode`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssNodeKind {
    /// A style rule, including a nested rule such as `&:hover { ... }`.
    StyleRule {
        /// Selector list without surrounding whitespace.
        selector: Range<usize>,
        /// Block contents.
        block: CssBlock,
    },
    /// An at-rule other than an `@apply` statement.
    AtRule {
        /// Name without `@`, in ASCII lowercase.
        name: LeanStr,
        /// Prelude without surrounding whitespace. It is empty when the at-rule has none.
        prelude: Range<usize>,
        /// Block contents, or `None` for a statement such as `@import "x.css";`.
        block: Option<CssBlock>,
    },
    /// An `@apply` statement without a block.
    Apply {
        /// Each candidate in authored order. Whitespace and comments separate
        /// candidates. Quoted strings and bracketed or parenthesized parts stay inside
        /// the candidate that contains them.
        candidates: Vec<Range<usize>>,
    },
    /// A declaration, including a custom property whose value contains braces.
    Declaration {
        /// Property name as written.
        property: Range<usize>,
        /// Value without surrounding whitespace, including any `!important`.
        value: Range<usize>,
    },
}

/// The contents of one `{ ... }` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssBlock {
    /// Range between the braces. For an unclosed block, it ends at the end of the input.
    pub contents: Range<usize>,
    /// Nodes in authored order.
    pub children: Vec<CssNode>,
}

/// Returns the top-level nodes of `css` in authored order.
///
/// Every range indexes `css`. Within a block, a sequence that starts with an identifier
/// and a colon and ends at `;` or at the end of the block is a declaration. A sequence
/// that reaches a block first is a style rule, except for a custom property, whose
/// value may contain blocks.
pub fn parse_structure(css: &str) -> Vec<CssNode> {
    parse_block(&mut Parser::new(css), css)
}

fn parse_block(parser: &mut Parser<'_>, css: &str) -> Vec<CssNode> {
    let mut nodes = Vec::new();
    loop {
        let state = parser.state();
        let start = parser.position().byte_index();
        let token = match parser.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(_) => break,
        };
        match token {
            Token::WhiteSpace(_)
            | Token::Comment(_)
            | Token::Semicolon
            | Token::CDO
            | Token::CDC => {}
            Token::AtKeyword(name) => {
                nodes.push(at_rule(
                    parser,
                    css,
                    start,
                    name.to_ascii_lowercase().into(),
                ));
            }
            _ => {
                parser.reset(&state);
                nodes.extend(rule_or_declaration(parser, css, start));
            }
        }
    }
    nodes
}

/// Reads an at-rule whose `@name` token ends at the parser position.
fn at_rule(parser: &mut Parser<'_>, css: &str, start: usize, name: LeanStr) -> CssNode {
    let prelude_start = parser.position().byte_index();
    let mut candidates = Vec::new();
    let mut candidate: Option<Range<usize>> = None;
    let (prelude_end, end, block) = loop {
        let before = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) => break (before, parser.position().byte_index(), None),
            Ok(Token::CurlyBracketBlock) => {
                let (block, end) = block(parser, css);
                break (before, end, Some(block));
            }
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {
                candidates.extend(candidate.take());
                continue;
            }
            Ok(Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock) => {
                skip_nested(parser);
            }
            Ok(_) => {}
            Err(_) => {
                let end = parser.position().byte_index();
                break (end, end, None);
            }
        }
        let after = parser.position().byte_index();
        candidate = Some(candidate.map_or(before, |range| range.start)..after);
    };
    candidates.extend(candidate);
    let prelude = trimmed(css, prelude_start..prelude_end);
    let kind = if name == "apply" && block.is_none() {
        CssNodeKind::Apply { candidates }
    } else {
        CssNodeKind::AtRule {
            name,
            prelude,
            block,
        }
    };
    let end = if end == prelude_end {
        prelude_end.min(trimmed(css, start..prelude_end).end)
    } else {
        end
    };
    CssNode {
        range: start..end,
        kind,
    }
}

/// Reads a style rule or a declaration that starts at the parser position.
fn rule_or_declaration(parser: &mut Parser<'_>, css: &str, start: usize) -> Option<CssNode> {
    let mut property = None;
    let mut colon = None;
    let mut significant = 0_usize;
    let (value_end, end) = loop {
        let before = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) => break (before, parser.position().byte_index()),
            Ok(Token::CurlyBracketBlock) => {
                let custom = colon.is_some()
                    && property
                        .as_ref()
                        .is_some_and(|range: &Range<usize>| css[range.clone()].starts_with("--"));
                if custom {
                    skip_nested(parser);
                    continue;
                }
                let (block, end) = block(parser, css);
                return Some(CssNode {
                    range: start..end,
                    kind: CssNodeKind::StyleRule {
                        selector: trimmed(css, start..before),
                        block,
                    },
                });
            }
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => continue,
            Ok(Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock) => {
                skip_nested(parser);
            }
            Ok(Token::Ident(_)) if significant == 0 => {
                property = Some(before..parser.position().byte_index());
            }
            Ok(Token::Colon) if significant == 1 && property.is_some() => {
                colon = Some(parser.position().byte_index());
            }
            Ok(_) => {}
            Err(_) => {
                let end = parser.position().byte_index();
                break (end, end);
            }
        }
        significant += 1;
    };
    let (Some(property), Some(colon)) = (property, colon) else {
        return None;
    };
    let value = trimmed(css, colon..value_end);
    Some(CssNode {
        range: start..if end == value_end { value.end } else { end },
        kind: CssNodeKind::Declaration { property, value },
    })
}

/// Reads the block whose `{` the parser just consumed and returns it with the offset
/// after its closing brace.
fn block(parser: &mut Parser<'_>, css: &str) -> (CssBlock, usize) {
    let contents_start = parser.position().byte_index();
    let children = parser
        .parse_nested_block(|nested| Ok::<_, ParseError<()>>(parse_block(nested, css)))
        .unwrap_or_default();
    let end = parser.position().byte_index();
    // cssparser ends an unclosed block at the end of the input. The last byte is then a
    // brace only when it closes the last child.
    let closed = end > contents_start
        && css.as_bytes().get(end - 1) == Some(&b'}')
        && children.last().is_none_or(|child| child.range.end < end);
    let contents_end = if closed { end - 1 } else { end };
    (
        CssBlock {
            contents: contents_start..contents_end,
            children,
        },
        end,
    )
}

/// Consumes the contents of the function or bracket block that the parser just opened.
fn skip_nested(parser: &mut Parser<'_>) {
    let _ = parser.parse_nested_block(|nested| {
        while nested.next_including_whitespace_and_comments().is_ok() {}
        Ok::<_, ParseError<()>>(())
    });
}

/// Returns `range` without leading and trailing ASCII whitespace.
fn trimmed(css: &str, range: Range<usize>) -> Range<usize> {
    let text = &css[range.clone()];
    let start = range.start + (text.len() - text.trim_start().len());
    let end = range.end - (text.len() - text.trim_end().len());
    start..end.max(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text<'a>(css: &'a str, range: &Range<usize>) -> &'a str {
        &css[range.clone()]
    }

    #[test]
    fn nested_rules_at_rules_and_declarations_keep_their_ranges() {
        let css = ".card { color: red; &:hover { opacity: 0.5 } @media print { color: black; } }";
        let nodes = parse_structure(css);
        assert_eq!(nodes.len(), 1);
        assert_eq!(text(css, &nodes[0].range), css);
        let CssNodeKind::StyleRule { selector, block } = &nodes[0].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(text(css, selector), ".card");
        assert_eq!(text(css, &block.contents), &css[7..css.len() - 1]);
        let children = nodes[0].children();
        assert_eq!(children.len(), 3);
        let CssNodeKind::Declaration { property, value } = &children[0].kind else {
            panic!("{children:?}");
        };
        assert_eq!((text(css, property), text(css, value)), ("color", "red"));
        assert_eq!(text(css, &children[0].range), "color: red;");
        let CssNodeKind::StyleRule { selector, .. } = &children[1].kind else {
            panic!("{children:?}");
        };
        assert_eq!(text(css, selector), "&:hover");
        assert_eq!(
            text(css, &children[1].children()[0].range),
            "opacity: 0.5",
            "an unterminated last declaration ends at its value"
        );
        let CssNodeKind::AtRule { name, prelude, .. } = &children[2].kind else {
            panic!("{children:?}");
        };
        assert_eq!((name.as_str(), text(css, prelude)), ("media", "print"));
    }

    #[test]
    fn apply_candidates_are_separated_by_whitespace_and_comments_only() {
        let css = ".x { @APPLY flex  hover:[content:'a_b;c'] /* note */ bg-(--x) ; }";
        let nodes = parse_structure(css);
        let CssNodeKind::Apply { candidates } = &nodes[0].children()[0].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(
            candidates
                .iter()
                .map(|range| text(css, range))
                .collect::<Vec<_>>(),
            ["flex", "hover:[content:'a_b;c']", "bg-(--x)"]
        );
        assert!(text(css, &nodes[0].children()[0].range).ends_with(';'));
        let nodes = parse_structure(".x { @apply flex }");
        let CssNodeKind::Apply { candidates } = &nodes[0].children()[0].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!((candidates.len(), &candidates[0]), (1, &(12..16)));
        assert_eq!(nodes[0].children()[0].range, 5..16);
    }

    #[test]
    fn comments_and_strings_never_start_a_node() {
        let css = "/* @apply a; .x { } */ .y { content: '@variant b { }'; } @variant c { --v: { a: b }; }";
        let nodes = parse_structure(css);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].children().len(), 1);
        let CssNodeKind::AtRule { name, block, .. } = &nodes[1].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(name, "variant");
        let children = &block.as_ref().unwrap().children;
        assert!(
            matches!(&children[0].kind, CssNodeKind::Declaration { value, .. } if text(css, value) == "{ a: b }"),
            "{children:?}"
        );
    }

    #[test]
    fn statements_and_unclosed_blocks_end_at_their_last_token() {
        let css = "@import 'a.css' layer(x); .a { b { color: red }";
        let nodes = parse_structure(css);
        assert!(matches!(
            &nodes[0].kind,
            CssNodeKind::AtRule { block: None, prelude, .. } if text(css, prelude) == "'a.css' layer(x)"
        ));
        let CssNodeKind::StyleRule { block, .. } = &nodes[1].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(block.contents.end, css.len());
        assert_eq!(nodes[1].range.end, css.len());
        assert_eq!(text(css, &block.children[0].range), "b { color: red }");
    }

    #[test]
    fn text_that_is_neither_a_rule_nor_a_declaration_has_no_node() {
        assert_eq!(parse_structure(".a { 12px; }")[0].children(), &[]);
        assert_eq!(parse_structure("a:hover; b { }").len(), 2);
    }
}
