//! The nesting selector `&`: finding it, counting it, inserting it, and replacing it.

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanString;

/// Whether a selector contains a nesting token referring to its current subject.
/// Quoted, escaped, commented, and attribute-value ampersands are literal text.
pub fn selector_uses_subject(selector: &str) -> bool {
    let mut positions = Vec::new();
    collect(&mut Parser::new(selector), &mut positions);
    !positions.is_empty()
}

/// Whether one complex selector contains more than one nesting token.
///
/// Each comma-separated alternative counts separately, including alternatives inside
/// functional pseudo-classes, so `:where(&:hover, &:focus)` uses `&` once and
/// `& + &` or `&:has(+ &)` uses it twice. Strings, comments, escaped identifiers and
/// attribute values do not count.
pub fn selector_repeats_subject(selector: &str) -> bool {
    subject_uses(&mut Parser::new(selector)) > 1
}

/// Returns the largest number of nesting tokens in one alternative of the list that
/// `parser` reads.
fn subject_uses(parser: &mut Parser<'_>) -> usize {
    let mut largest = 0;
    let mut current = 0;
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::Delim('&') => current += 1,
            Token::Comma => {
                largest = largest.max(current);
                current = 0;
            }
            Token::Function(_) | Token::ParenthesisBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    current += subject_uses(inner);
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
    largest.max(current)
}

/// Returns the selector that a nested style rule selector denotes under CSS Nesting.
///
/// Each comma-separated branch that starts with a combinator (`>`, `+`, or `~`) or
/// that contains no `&` gets `& ` in front of it, so `:hover` becomes `& :hover` and
/// `> .item` becomes `& > .item`. Other branches, whitespace and comments stay as
/// written. Quoted, escaped, commented, and attribute-value ampersands are literal text
/// and do not count as `&`.
pub fn absolutize_nested_selector(selector: &str) -> LeanString {
    let mut parser = Parser::new(selector);
    let mut insertions = Vec::new();
    let mut branch: Option<(usize, bool)> = None;
    let mut uses_subject = false;
    loop {
        let start = parser.position().byte_index();
        let (token, end) = match parser.next_including_whitespace_and_comments() {
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => continue,
            Ok(Token::Comma) => (None, false),
            Ok(token) => (Some(token.clone()), false),
            Err(_) => (None, true),
        };
        let Some(token) = token else {
            if let Some((position, relative)) = branch.take()
                && (relative || !uses_subject)
            {
                insertions.push(position);
            }
            uses_subject = false;
            if end {
                break;
            }
            continue;
        };
        branch.get_or_insert((start, matches!(token, Token::Delim('>' | '+' | '~'))));
        match token {
            Token::Delim('&') => uses_subject = true,
            Token::Function(_) | Token::ParenthesisBlock => {
                let mut positions = Vec::new();
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    collect(inner, &mut positions);
                    Ok(())
                });
                uses_subject |= !positions.is_empty();
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
    let mut output = LeanString::new();
    let mut cursor = 0;
    for position in insertions {
        output.push_str(&selector[cursor..position]);
        output.push_str("& ");
        cursor = position;
    }
    output.push_str(&selector[cursor..]);
    output
}

/// Emits a selector template with CSS nesting tokens bound to `parent`.
/// Strings, comments, escaped identifiers and attribute values remain literal.
pub fn instantiate_selector(template: &str, parent: &str) -> LeanString {
    let mut positions = Vec::new();
    collect(&mut Parser::new(template), &mut positions);
    let mut output = LeanString::new();
    let mut cursor = 0;
    for position in positions {
        output.push_str(&template[cursor..position]);
        output.push_str(parent);
        cursor = position + 1;
    }
    output.push_str(&template[cursor..]);
    output
}

/// Rewrites a selector template as a selector that a functional pseudo-class such as
/// `:not()` evaluates against the subject itself.
///
/// Each nesting token becomes the universal selector `*`. The `*` is omitted when a
/// class, ID, attribute, or pseudo-class selector follows directly in the same
/// compound, so `&:hover` becomes `:hover` and `& .item` becomes `* .item`. Strings,
/// comments, escaped identifiers and attribute values remain literal.
pub fn subject_as_universal(template: &str) -> LeanString {
    let mut positions = Vec::new();
    collect(&mut Parser::new(template), &mut positions);
    let mut output = LeanString::new();
    let mut cursor = 0;
    for position in positions {
        output.push_str(&template[cursor..position]);
        if !matches!(
            template.as_bytes().get(position + 1),
            Some(b':' | b'.' | b'#' | b'[')
        ) {
            output.push('*');
        }
        cursor = position + 1;
    }
    output.push_str(&template[cursor..]);
    output
}

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

    #[test]
    fn subject_becomes_universal_only_where_the_compound_needs_a_type() {
        for (template, expected) in [
            ("&:hover", ":hover"),
            ("& .item", "* .item"),
            (".group &", ".group *"),
            ("&", "*"),
            ("&.on, &[open], &#id", ".on, [open], #id"),
            (r#":is(&)[data-x="&"] .\&"#, r#":is(*)[data-x="&"] .\&"#),
        ] {
            assert_eq!(subject_as_universal(template), expected, "{template}");
        }
    }

    #[test]
    fn repeated_subjects_are_counted_per_complex_selector() {
        for (selector, expected) in [
            ("&:hover", false),
            ("&:hover, &:focus", false),
            (":where(&:hover, &:focus)", false),
            (r#"&[data-x="&"] .\&"#, false),
            ("& + &", true),
            ("& &", true),
            ("&:hover, & ~ &", true),
            ("&:has(+ &)", true),
            (":is(:where(.group) + & *)&", true),
        ] {
            assert_eq!(selector_repeats_subject(selector), expected, "{selector}");
        }
    }

    #[test]
    fn nested_selectors_gain_a_leading_subject_unless_they_use_one() {
        for (selector, expected) in [
            (":hover", "& :hover"),
            (".open", "& .open"),
            ("> .item", "& > .item"),
            ("+.x", "& +.x"),
            ("~ .x", "& ~ .x"),
            (":hover, .open", "& :hover, & .open"),
            (" :hover ,	.open", " & :hover ,	& .open"),
            ("&:hover", "&:hover"),
            (".parent &", ".parent &"),
            (":is(&:hover)", ":is(&:hover)"),
            ("> &", "& > &"),
            ("&:hover, .open", "&:hover, & .open"),
            (
                r#"[data-x="&"] .\&, /* & */ .a"#,
                r#"& [data-x="&"] .\&, /* & */ & .a"#,
            ),
            (":not(.a, .b)", "& :not(.a, .b)"),
        ] {
            assert_eq!(absolutize_nested_selector(selector), expected, "{selector}");
        }
    }
}
