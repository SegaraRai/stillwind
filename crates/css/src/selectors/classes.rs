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
    for test in selector_class_tests(selector) {
        if test.matcher == ClassMatcher::Name {
            push(&test.value, &mut names);
        }
    }
    names
}

/// How a selector compares the `class` attribute with a value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassMatcher {
    /// `.value` or `[class~=value]`: one whole class name.
    Name,
    /// `[class=value]`: the whole attribute text.
    Exact,
    /// `[class^=value]`: the start of the attribute text.
    Prefix,
    /// `[class$=value]`: the end of the attribute text.
    Suffix,
    /// `[class*=value]`: any part of the attribute text.
    Substring,
    /// `[class|=value]`: the attribute text, or its start followed by `-`.
    DashPrefix,
}

/// One test of the `class` attribute in a selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassTest {
    /// Comparison that the selector makes.
    pub matcher: ClassMatcher,
    /// Compared value, decoded from CSS escapes and quotes.
    pub value: LeanStr,
    /// Byte range of the class selector or attribute selector in the selector text,
    /// including the `.` or the brackets.
    pub range: std::ops::Range<usize>,
}

/// Returns every test of the `class` attribute in `selector`, in source order, including
/// tests inside functional pseudo-classes. Attribute names are compared without case.
/// The caller must pass selector text only, as for [`selector_class_names`].
pub fn selector_class_tests(selector: &str) -> Vec<ClassTest> {
    let mut tests = Vec::new();
    collect(&mut Parser::new(selector), &mut tests);
    tests
}

fn push(name: &str, names: &mut Vec<LeanStr>) {
    if !name.is_empty() && !names.iter().any(|existing| existing == name) {
        names.push(name.into());
    }
}

fn collect(parser: &mut Parser<'_>, tests: &mut Vec<ClassTest>) {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        match token {
            Token::Delim('.') => {
                let state = parser.state();
                match parser.next_including_whitespace_and_comments() {
                    Ok(Token::Ident(name)) => {
                        let value: LeanStr = name.as_ref().into();
                        if !value.is_empty() {
                            tests.push(ClassTest {
                                matcher: ClassMatcher::Name,
                                value,
                                range: start..parser.position().byte_index(),
                            });
                        }
                    }
                    _ => {
                        parser.reset(&state);
                    }
                }
            }
            Token::SquareBracketBlock => {
                let test: Result<_, ParseError<()>> = parser.parse_nested_block(|inner| {
                    let test = attribute_class_test(inner);
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(test)
                });
                if let Ok(Some((matcher, value))) = test {
                    tests.push(ClassTest {
                        matcher,
                        value,
                        range: start..parser.position().byte_index(),
                    });
                }
            }
            Token::Function(_) | Token::ParenthesisBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    collect(inner, tests);
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

/// Returns the comparison and value of an attribute selector body that tests `class`,
/// such as `class*="size-"`, ignoring the case of the attribute name, or `None` for any
/// other attribute selector.
fn attribute_class_test(parser: &mut Parser<'_>) -> Option<(ClassMatcher, LeanStr)> {
    let attribute = parser.expect_ident().ok()?;
    if !attribute.eq_ignore_ascii_case("class") {
        return None;
    }
    let matcher = match parser.next().ok()? {
        Token::IncludeMatch => ClassMatcher::Name,
        Token::Delim('=') => ClassMatcher::Exact,
        Token::PrefixMatch => ClassMatcher::Prefix,
        Token::SuffixMatch => ClassMatcher::Suffix,
        Token::SubstringMatch => ClassMatcher::Substring,
        Token::DashMatch => ClassMatcher::DashPrefix,
        _ => return None,
    };
    match parser.next().ok()? {
        Token::Ident(value) | Token::QuotedString(value) => Some((matcher, value.as_ref().into())),
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
    fn reports_every_class_attribute_comparison_with_its_range() {
        let selector = r"& svg:not([class*='size-']):is([CLASS^=text-], .a)";
        let tests = selector_class_tests(selector);
        assert_eq!(
            tests
                .iter()
                .map(|test| (
                    test.matcher,
                    test.value.as_str(),
                    &selector[test.range.clone()]
                ))
                .collect::<Vec<_>>(),
            [
                (ClassMatcher::Substring, "size-", "[class*='size-']"),
                (ClassMatcher::Prefix, "text-", "[CLASS^=text-]"),
                (ClassMatcher::Name, "a", ".a"),
            ]
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
