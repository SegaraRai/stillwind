//! CSS token comparisons for generated declarations.

use cssparser::{ParseError, Parser, Token};

use stillwind_css::fold_calc_expressions;

/// Asserts that two CSS value or declaration sequences have equal tokens after
/// arithmetic evaluation. Evaluates each argument once and ignores whitespace and
/// comments while preserving strings, token boundaries, and declaration order.
///
/// # Panics
///
/// Panics if the normalized token sequences differ.
#[macro_export]
macro_rules! assert_css_eq {
    ($actual:expr, $expected:expr $(,)?) => {
        ::std::assert_eq!(
            $crate::assertions::css_tokens(&$actual),
            $crate::assertions::css_tokens(&$expected),
        );
    };
}

/// Compares value tokens after test-only arithmetic evaluation. Whitespace inside
/// strings and token boundaries remain observable; declaration order is never sorted.
pub fn css_tokens(value: &str) -> Vec<String> {
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
    let value = fold_calc_expressions(value, None);
    let mut output = Vec::new();
    collect(&mut Parser::new(&value), &mut output);
    output
}

/// Tests whether a CSS string contains a contiguous sequence of value tokens.
pub trait CssAssertions {
    /// Returns whether `expected` occurs after arithmetic evaluation of both strings.
    /// Whitespace and comments are ignored, and an empty token sequence matches.
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

#[test]
fn equality_assertion_evaluates_each_argument_once() {
    let calls = std::cell::Cell::new(0);
    assert_css_eq!(
        {
            calls.set(calls.get() + 1);
            String::from("width:calc(1px + 2px);")
        },
        {
            calls.set(calls.get() + 1);
            "width:3px;"
        },
    );
    assert_eq!(calls.get(), 2);
}
