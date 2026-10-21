//! Class names that a selector requires on some element.

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanStr;

/// Returns the class names that `selector` matches by name, in source order.
///
/// A class selector `.name` and an attribute selector `[class~="name"]` both match one
/// whole class name, so both count. Names inside functional pseudo-classes such as
/// `:not()`, `:is()`, and `:has()` count, because they still require an element with
/// that class or test for one. Names are decoded from CSS escapes. Repeated names are
/// returned once.
///
/// Partial attribute matches such as `[class*="name"]` and `[class^="name"]` do not
/// name one class and are skipped. The caller must pass selector text only: a
/// declaration value or an at-rule prelude can contain `.name` text that is not a
/// class selector.
pub fn selector_class_names(selector: &str) -> Vec<LeanStr> {
    let mut names = Vec::new();
    collect(&mut Parser::new(selector), &mut names);
    names
}

fn push(name: &str, names: &mut Vec<LeanStr>) {
    if !name.is_empty() && !names.iter().any(|existing| existing == name) {
        names.push(name.into());
    }
}

fn collect(parser: &mut Parser<'_>, names: &mut Vec<LeanStr>) {
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::Delim('.') => {
                let state = parser.state();
                match parser.next_including_whitespace_and_comments() {
                    Ok(Token::Ident(name)) => {
                        let name = name.clone();
                        push(&name, names);
                    }
                    _ => {
                        parser.reset(&state);
                    }
                }
            }
            Token::SquareBracketBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    if let Some(name) = attribute_class_name(inner) {
                        push(&name, names);
                    }
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
            }
            Token::Function(_) | Token::ParenthesisBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    collect(inner, names);
                    Ok(())
                });
            }
            Token::CurlyBracketBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
            }
            _ => {}
        }
    }
}

/// Returns the class name of an attribute selector body `class~="name"`, ignoring the
/// case of the attribute name, or `None` for any other attribute selector.
fn attribute_class_name(parser: &mut Parser<'_>) -> Option<LeanStr> {
    let attribute = parser.expect_ident().ok()?;
    if !attribute.eq_ignore_ascii_case("class") {
        return None;
    }
    if !matches!(parser.next().ok()?, Token::IncludeMatch) {
        return None;
    }
    match parser.next().ok()? {
        Token::Ident(value) | Token::QuotedString(value) => Some(value.as_ref().into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(selector: &str) -> Vec<String> {
        selector_class_names(selector)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn collects_class_selectors_at_every_position() {
        assert_eq!(names(".a &"), ["a"]);
        assert_eq!(
            names("&:not(.b) :is(.c, .d):has(> .e)"),
            ["b", "c", "d", "e"]
        );
        assert_eq!(names(".a.b, .a .c"), ["a", "b", "c"]);
        assert_eq!(names(r".foo\/bar &"), ["foo/bar"]);
    }

    #[test]
    fn collects_whole_class_attribute_matches_only() {
        assert_eq!(
            names(r#":where([class~="not-prose"],[CLASS~=x i] *)"#),
            ["not-prose", "x"]
        );
        assert_eq!(
            names(r#"[class*="a"], [class^=b], [class="c"], [data-x~="d"]"#),
            [] as [&str; 0]
        );
    }

    #[test]
    fn ignores_strings_comments_and_ids() {
        assert_eq!(
            names(r#"[data-x=".a"] /* .b */ #c &::before"#),
            [] as [&str; 0]
        );
    }
}
