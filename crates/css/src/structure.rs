//! Rule structure of authored stylesheets.
//!
//! [`parse_structure`] tokenizes CSS with cssparser and returns the style rules,
//! at-rules, and declarations of each block, with byte ranges into the input. Comments
//! and strings never start a node, so directive-shaped text inside them is not a
//! directive. The compiler expands `@apply` and `@variant` through this model, and
//! stylesheet lint reports at its ranges, so both agree on which text is a directive.
//!
//! The parser accepts malformed input. An unclosed block ends at the end of the input,
//! and text that is neither a rule nor a declaration contributes no node. Callers that
//! need validation parse the stylesheet separately.

use std::ops::Range;

use cssparser::{Parser, Token};
use lean_string::LeanStr;

use crate::parser::{CssBlockRange, consume_css_block, parse_css_block};

/// One rule, at-rule, or declaration with its byte range.
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
    /// An at-rule, with its uninterpreted prelude.
    AtRule {
        /// Name without `@`, in ASCII lowercase. Empty for a lone `@` in incomplete
        /// input.
        name: LeanStr,
        /// Prelude without surrounding whitespace. It is empty when the at-rule has
        /// none.
        prelude: Range<usize>,
        /// Block contents, or `None` for a statement such as `@import "x.css";`.
        block: Option<CssBlock>,
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
    /// Range between the braces. For an unclosed block, it ends at the end of the
    /// input.
    pub contents: Range<usize>,
    /// Byte offset of the closing brace, or None when EOF closes the block.
    pub close: Option<usize>,
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
            Token::Delim('@') => {
                nodes.push(at_rule(parser, css, start, LeanStr::default()));
            }
            _ => {
                parser.reset(&state);
                nodes.extend(rule_or_declaration(parser, css, start));
            }
        }
    }
    nodes
}

/// Reads an at-rule after its `@name` token or an incomplete lone `@`.
fn at_rule(parser: &mut Parser<'_>, css: &str, start: usize, name: LeanStr) -> CssNode {
    let prelude_start = parser.position().byte_index();
    let (prelude_end, end, block) = loop {
        let before = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) => {
                break (before, parser.position().byte_index(), None);
            }
            Ok(Token::CurlyBracketBlock) => {
                let (block, end) = block(parser, css);
                break (before, end, Some(block));
            }
            Ok(Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock) => {
                let _ = consume_css_block(parser);
            }
            Ok(_) => {}
            Err(_) => {
                let end = parser.position().byte_index();
                break (end, end, None);
            }
        }
    };
    let prelude = trimmed(css, prelude_start..prelude_end);
    let kind = CssNodeKind::AtRule {
        name,
        prelude,
        block,
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
            Ok(Token::Semicolon) => {
                break (before, parser.position().byte_index());
            }
            Ok(Token::CurlyBracketBlock) => {
                let custom = colon.is_some()
                    && property
                        .as_ref()
                        .is_some_and(|range: &Range<usize>| css[range.clone()].starts_with("--"));
                if custom {
                    let _ = consume_css_block(parser);
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
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {
                continue;
            }
            Ok(Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock) => {
                let _ = consume_css_block(parser);
            }
            Ok(Token::Ident(_)) if significant == 0 => {
                property = Some(before..parser.position().byte_index());
            }
            Ok(Token::Colon)
                if colon.is_none()
                    && property.is_some()
                    && (significant == 1 || css[start..before].trim_start().starts_with("--")) =>
            {
                if significant > 1 {
                    property = Some(trimmed(css, start..before));
                }
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
    let start = parser.position().byte_index();
    let (children, bounds) = parse_css_block(parser, |inner| parse_block(inner, css))
        .unwrap_or_else(|_| {
            (
                Vec::new(),
                CssBlockRange {
                    contents: start..start,
                    close: None,
                },
            )
        });
    let end = parser.position().byte_index();
    (
        CssBlock {
            contents: bounds.contents,
            close: bounds.close,
            children,
        },
        end,
    )
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

    #[test]
    fn at_rule_preludes_preserve_raw_tokens_and_optional_semicolons() {
        let css = ".x { @APPLY flex  hover:[content:'a_b;c'] /* note */ bg-(--x) ; }";
        let nodes = parse_structure(css);
        let CssNodeKind::AtRule {
            name,
            prelude,
            block,
        } = &nodes[0].children()[0].kind
        else {
            panic!("{nodes:?}");
        };
        assert_eq!(name, "apply");
        assert_eq!(
            &css[prelude.clone()],
            "flex  hover:[content:'a_b;c'] /* note */ bg-(--x)"
        );
        assert!(block.is_none());
        assert!(css[nodes[0].children()[0].range.clone()].ends_with(';'));
        let nodes = parse_structure(".x { @apply flex }");
        let CssNodeKind::AtRule { prelude, .. } = &nodes[0].children()[0].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(prelude, &(12..16));
        assert_eq!(nodes[0].children()[0].range, 5..16);
    }

    #[test]
    fn node_ranges_end_at_top_level_declarations_and_rule_blocks() {
        for (css, expected) in [
            (
                "content:'a;b';x:f(a;b);last:1",
                vec!["content:'a;b';", "x:f(a;b);", "last:1"],
            ),
            (" ", vec![]),
            (
                "@layer x{color:red;gap:0;}top:0;--x:{a;b};last:1",
                vec![
                    "@layer x{color:red;gap:0;}",
                    "top:0;",
                    "--x:{a;b};",
                    "last:1",
                ],
            ),
        ] {
            assert_eq!(
                parse_structure(css)
                    .into_iter()
                    .map(|node| &css[node.range])
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn nested_rules_at_rules_and_declarations_keep_their_ranges() {
        let css = ".card { color: red; &:hover { opacity: 0.5 } @media print { color: black; } }";
        let nodes = parse_structure(css);
        assert_eq!(nodes.len(), 1);
        assert_eq!(&css[nodes[0].range.clone()], css);
        let CssNodeKind::StyleRule { selector, block } = &nodes[0].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(&css[selector.clone()], ".card");
        assert_eq!(&css[block.contents.clone()], &css[7..css.len() - 1]);
        let children = nodes[0].children();
        assert_eq!(children.len(), 3);
        let CssNodeKind::Declaration { property, value } = &children[0].kind else {
            panic!("{children:?}");
        };
        assert_eq!(
            (&css[property.clone()], &css[value.clone()]),
            ("color", "red")
        );
        assert_eq!(&css[children[0].range.clone()], "color: red;");
        let CssNodeKind::StyleRule { selector, .. } = &children[1].kind else {
            panic!("{children:?}");
        };
        assert_eq!(&css[selector.clone()], "&:hover");
        assert_eq!(
            &css[children[1].children()[0].range.clone()],
            "opacity: 0.5",
            "an unterminated last declaration ends at its value"
        );
        let CssNodeKind::AtRule { name, prelude, .. } = &children[2].kind else {
            panic!("{children:?}");
        };
        assert_eq!((name.as_str(), &css[prelude.clone()]), ("media", "print"));
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
            matches!(&children[0].kind, CssNodeKind::Declaration { value, .. } if &css[value.clone()] == "{ a: b }"),
            "{children:?}"
        );
    }

    #[test]
    fn statements_and_unclosed_blocks_end_at_their_last_token() {
        let css = "@import 'a.css' layer(x); .a { b { color: red }";
        let nodes = parse_structure(css);
        assert!(matches!(
            &nodes[0].kind,
            CssNodeKind::AtRule { block: None, prelude, .. } if &css[prelude.clone()] == "'a.css' layer(x)"
        ));
        let CssNodeKind::StyleRule { block, .. } = &nodes[1].kind else {
            panic!("{nodes:?}");
        };
        assert_eq!(block.contents.end, css.len());
        assert_eq!(nodes[1].range.end, css.len());
        assert_eq!(&css[block.children[0].range.clone()], "b { color: red }");
    }

    #[test]
    fn text_that_is_neither_a_rule_nor_a_declaration_has_no_node() {
        assert_eq!(parse_structure(".a { 12px; }")[0].children(), &[]);
        assert_eq!(parse_structure("a:hover; b { }").len(), 2);
    }
}
