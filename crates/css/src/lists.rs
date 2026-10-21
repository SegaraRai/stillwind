//! Splitting of component value lists at top-level tokens.

use std::ops::Range;

use cssparser::{Parser, Token};

use crate::parser::{closer, consume_css_block};

/// Splits `text` at the top-level tokens that `separator` accepts and removes them.
///
/// Tokens inside blocks and functions never separate. The items keep their whitespace
/// and comments, and an empty `text` returns one empty item.
///
/// The parser skips the contents of a block only when it reads the next token, so this
/// function consumes each block before it records the start of the following token.
/// Otherwise a separator after a block, as in `(a), (b)`, would split inside the block.
pub fn split_top_level(text: &str, separator: impl Fn(&Token<'_>) -> bool) -> Vec<&str> {
    let mut parser = Parser::new(text);
    let mut items = Vec::new();
    let mut start = 0;
    loop {
        let position = parser.position().byte_index();
        let (separates, block) = match parser.next_including_whitespace_and_comments() {
            Ok(token) => (
                separator(token),
                matches!(
                    token,
                    Token::ParenthesisBlock
                        | Token::SquareBracketBlock
                        | Token::CurlyBracketBlock
                        | Token::Function(_)
                ),
            ),
            Err(_) => break,
        };
        if separates {
            items.push(&text[start..position]);
            start = parser.position().byte_index();
        } else if block {
            let _ = consume_css_block(&mut parser);
        }
    }
    items.push(&text[start..]);
    items
}

/// Splits at the first top-level token accepted by the separator, preserving raw text.
/// Nested blocks, strings, comments, and escaped delimiters never separate.
pub fn split_once_top_level(
    text: &str,
    separator: impl Fn(&Token<'_>) -> bool,
) -> (&str, Option<&str>) {
    let mut parser = Parser::new(text);
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return (text, None);
        };
        let separates = separator(token);
        let block = closer(token).is_some();
        if separates {
            return (
                &text[..start],
                Some(&text[parser.position().byte_index()..]),
            );
        }
        if block {
            let _ = consume_css_block(&mut parser);
        }
    }
}

/// Returns ranges of components separated by top-level CSS whitespace or comments.
/// Quoted strings and nested blocks remain inside their component.
pub fn whitespace_separated_ranges(text: &str) -> Vec<Range<usize>> {
    split_top_level(text, |token| {
        matches!(token, Token::WhiteSpace(_) | Token::Comment(_))
    })
    .into_iter()
    .filter(|part| !part.is_empty())
    .map(|part| {
        let start = part.as_ptr().addr() - text.as_ptr().addr();
        start..start + part.len()
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_separator_preserves_nested_quoted_and_escaped_components() {
        for (input, expected) in [
            (
                "var(--a, red), rgb(1, 2, 3)",
                ("var(--a, red)", Some(" rgb(1, 2, 3)")),
            ),
            ("'a,b'", ("'a,b'", None)),
            (r"foo\,bar,baz", (r"foo\,bar", Some("baz"))),
            ("[a,b],x,y", ("[a,b]", Some("x,y"))),
            (r"{a:b,c},foo\,bar", ("{a:b,c}", Some(r"foo\,bar"))),
            ("", ("", None)),
        ] {
            assert_eq!(
                split_once_top_level(input, |token| matches!(token, Token::Comma)),
                expected
            );
        }
    }

    #[test]
    fn whitespace_ranges_preserve_strings_blocks_and_name_escapes() {
        let input = r"flex  hover:[content:'a_b;c'] /* note */ bg-(--x) probe-\31 x";
        assert_eq!(
            whitespace_separated_ranges(input)
                .into_iter()
                .map(|range| &input[range])
                .collect::<Vec<_>>(),
            [
                "flex",
                "hover:[content:'a_b;c']",
                "bg-(--x)",
                r"probe-\31 x"
            ]
        );
    }

    #[test]
    fn separators_inside_blocks_and_functions_do_not_split() {
        let commas = |text| split_top_level(text, |token| matches!(token, Token::Comma));
        assert_eq!(commas("(a: 1), (b: 2)"), ["(a: 1)", " (b: 2)"]);
        assert_eq!(commas("f(a, b), [c, d] , e"), ["f(a, b)", " [c, d] ", " e"]);
        assert_eq!(commas(""), [""]);
        let and = |text| {
            split_top_level(
                text,
                |token| matches!(token, Token::Ident(name) if name.eq_ignore_ascii_case("and")),
            )
        };
        assert_eq!(and("(a) and ((b) and (c))"), ["(a) ", " ((b) and (c))"]);
    }
}
