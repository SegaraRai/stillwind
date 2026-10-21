//! Comparable CSS fragments with whitespace and ASCII case normalized by token kind.

use cssparser::{Parser, Token};
use lean_string::LeanString;

use crate::parser::{closer, parse_css_block};

/// ASCII case folding outside strings, URLs, and escaped tokens.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum CssCase {
    /// Keeps authored case.
    Keep,
    /// Lowercases pseudo-class and pseudo-element names.
    PseudoClasses,
    /// Lowercases unescaped tokens.
    All,
}

/// Removes comments and collapses whitespace in `text`, keeping strings and URLs.
/// Spaces next to parentheses, brackets, and `spacing` characters are removed.
/// Incomplete input retains its actual delimiters. `case` controls ASCII case folding.
pub fn normalize_css_fragment(text: &str, spacing: &[char], case: CssCase) -> LeanString {
    let mut output = LeanString::new();
    normalize(&mut Parser::new(text), text, spacing, case, &mut output);
    output
}

/// Appends normalized tokens and nested contents to `output`. `text` is the complete
/// source of `parser`, so literal tokens can retain their original bytes and case.
fn normalize(
    parser: &mut Parser<'_>,
    text: &str,
    spacing: &[char],
    case: CssCase,
    output: &mut LeanString,
) {
    let mut space = false;
    let mut pseudo = false;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            return;
        };
        let end = parser.position().byte_index();
        match token {
            Token::Comment(_) => {
                continue;
            }
            Token::WhiteSpace(_) => {
                space = !output.is_empty();
                continue;
            }
            _ => {}
        }
        let raw = &text[start..end];
        let first = raw.chars().next();
        if space
            && !output
                .chars()
                .last()
                .is_some_and(|last| matches!(last, '(' | '[') || spacing.contains(&last))
            && !first.is_some_and(|first| matches!(first, ')' | ']') || spacing.contains(&first))
        {
            output.push(' ');
        }
        space = false;
        let literal = matches!(
            token,
            Token::QuotedString(_) | Token::BadString(_) | Token::UnquotedUrl(_) | Token::BadUrl(_)
        );
        let fold = !literal
            && !raw.contains('\\')
            && (case == CssCase::All || (case == CssCase::PseudoClasses && pseudo));
        if fold {
            output.push_str(&raw.to_ascii_lowercase());
        } else {
            output.push_str(raw);
        }
        pseudo = matches!(token, Token::Colon);
        if closer(&token).is_some() {
            let result = parse_css_block(parser, |inner| {
                normalize(inner, text, spacing, case, output);
            });
            if let Ok(((), block)) = result
                && let Some(close) = block.close
            {
                output.push_str(&text[close..=close]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_literal_contents_and_real_eof_boundaries() {
        assert_eq!(
            normalize_css_fragment(
                r#"&:HOVER [data-x="A : )"] /* ) */ > .Item"#,
                &['>'],
                CssCase::PseudoClasses
            ),
            r#"&:hover [data-x="A : )"]>.Item"#
        );
        assert_eq!(
            normalize_css_fragment("( WIDTH : 4REM )", &[':'], CssCase::All),
            "(width:4rem)"
        );
        assert_eq!(
            normalize_css_fragment("( width: x(", &[':'], CssCase::Keep),
            "(width:x("
        );
    }
}
