//! Validity of selector lists and their top-level branches.

use cssparser::{ParseError, Parser, Token};
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::ParseWithOptions;

/// Whether `selector` is a valid selector list, in which `&` may appear anywhere a
/// compound selector may.
///
/// Unknown pseudo-classes and pseudo-elements with valid syntax are accepted, because
/// browsers may support selectors that the parser does not know. A branch that starts
/// with a combinator is rejected.
pub fn selector_is_valid(selector: &str) -> bool {
    SelectorList::parse_string_with_options(selector, ParserOptions::default()).is_ok()
}

/// Splits a selector list at its top-level commas and returns each branch without
/// surrounding whitespace.
///
/// Commas inside functions, blocks, strings, and escapes belong to their branch.
pub fn selector_list_branches(selector: &str) -> Vec<&str> {
    let mut parser = Parser::new(selector);
    let mut branches = Vec::new();
    let mut start = 0;
    loop {
        let position = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Comma) => {
                branches.push(selector[start..position].trim());
                start = parser.position().byte_index();
            }
            Ok(
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock,
            ) => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    branches.push(selector[start..].trim());
    branches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_lists_split_only_at_top_level_commas() {
        assert_eq!(
            selector_list_branches(":hover, :is(a, b) ,[x=\",\"]"),
            [":hover", ":is(a, b)", "[x=\",\"]"]
        );
        assert_eq!(selector_list_branches(":root"), [":root"]);
    }

    #[test]
    fn valid_selectors_allow_nesting_and_unknown_pseudo_classes() {
        for (selector, expected) in [
            ("&:hover", true),
            (".a, .b &", true),
            (":is(&, [data-value='&'])", true),
            ("&::-webkit-details-marker", true),
            (":future-state(x)", true),
            ("::view-transition-old(*)", true),
            ("--x:", false),
            ("color:", false),
            ("color:red;", false),
            ("&:", false),
            ("&;", false),
            (".a,", false),
            ("> img", false),
            ("", false),
        ] {
            assert_eq!(selector_is_valid(selector), expected, "{selector}");
        }
    }
}
