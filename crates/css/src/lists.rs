//! Splitting of component value lists at top-level tokens.

use cssparser::{ParseError, Parser, Token};

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
            let _ = parser.parse_nested_block(|block| {
                while block.next_including_whitespace_and_comments().is_ok() {}
                Ok::<(), ParseError<()>>(())
            });
        }
    }
    items.push(&text[start..]);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

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
