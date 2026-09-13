use cssparser::{ParseError, Parser, Token};

use stillwind_css::fold_calc_expressions;

/// Compares value tokens after test-only arithmetic evaluation. Whitespace inside
/// strings and token boundaries remain observable; declaration order is never sorted.
pub(crate) fn css_tokens(value: &str) -> Vec<String> {
    fn collect(parser: &mut Parser<'_>, output: &mut Vec<String>) {
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            let nested = matches!(
                token,
                Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock
            );
            if !matches!(token, Token::WhiteSpace(_) | Token::Comment(_)) {
                output.push(format!("{token:?}"));
            }
            if nested {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    collect(inner, output);
                    Ok(())
                });
                output.push("end-block".into());
            }
        }
    }
    let value = fold_calc_expressions(value);
    let mut output = Vec::new();
    collect(&mut Parser::new(&value), &mut output);
    output
}

pub(crate) trait CssAssertions {
    fn contains_css(&self, expected: &str) -> bool;
}

impl CssAssertions for str {
    fn contains_css(&self, expected: &str) -> bool {
        let actual = css_tokens(self);
        let expected = css_tokens(expected);
        expected.is_empty()
            || actual
                .windows(expected.len())
                .any(|tokens| tokens == expected)
    }
}

#[test]
fn comparison_preserves_strings_token_boundaries_fallbacks_and_order() {
    assert_eq!(
        css_tokens("width:calc(1px + 2px);"),
        css_tokens("width:3px;")
    );
    for (left, right) in [
        ("content:'a b';", "content:'ab';"),
        ("--x:1 2;", "--x:12;"),
        ("x:var(--v,);", "x:var(--v);"),
        ("color:red;color:blue;", "color:blue;color:red;"),
    ] {
        assert_ne!(css_tokens(left), css_tokens(right));
    }
}
