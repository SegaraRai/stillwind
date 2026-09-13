//! Expansion of nesting selector templates at rule construction time.

use cssparser::{ParseError, Parser, Token};

/// Emits a selector template with CSS nesting tokens bound to `parent`.
/// Strings, comments, escaped identifiers and attribute values remain literal.
pub fn instantiate_selector(template: &str, parent: &str) -> String {
    fn collect(parser: &mut Parser<'_>, positions: &mut Vec<usize>) {
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments() else {
                return;
            };
            match token {
                Token::Delim('&') => positions.push(start),
                Token::Function(_) | Token::ParenthesisBlock => {
                    let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                        collect(inner, positions);
                        Ok(())
                    });
                }
                Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                        while inner.next_including_whitespace_and_comments().is_ok() {}
                        Ok(())
                    });
                }
                _ => {}
            }
        }
    }

    let mut positions = Vec::new();
    collect(&mut Parser::new(template), &mut positions);
    let mut output = String::new();
    let mut cursor = 0;
    for position in positions {
        output.push_str(&template[cursor..position]);
        output.push_str(parent);
        cursor = position + 1;
    }
    output.push_str(&template[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn substitutes_only_nesting_tokens_without_reinterpreting_the_parent() {
        assert_eq!(
            instantiate_selector(r#":is(&, &[data-label="&"]) .\& /* & */ &"#, ".parent"),
            r#":is(.parent, .parent[data-label="&"]) .\& /* & */ .parent"#
        );
        assert_eq!(
            instantiate_selector("& + &", r#"[data-value="&"]"#),
            r#"[data-value="&"] + [data-value="&"]"#
        );
    }
}
