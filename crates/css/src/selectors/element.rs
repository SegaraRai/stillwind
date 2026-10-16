//! Conditions on the element that a selector selects: inserting one before its
//! pseudo-elements, and checking whether a selector can serve as one.

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanString;

/// Adds `condition` to the final element compound of each selector branch.
///
/// The caller must supply a valid selector and a compound selector fragment such as
/// `:is(.scope *)`. The fragment is inserted before the final compound's first
/// pseudo-element, so the condition tests its originating element. Selector lists,
/// whitespace, comments and literal text are preserved.
pub fn append_element_condition(selector: &str, condition: &str) -> LeanString {
    let mut parser = Parser::new(selector);
    let mut positions = Vec::new();
    let mut pseudo = None;
    let mut colon = None;
    let mut separated = false;
    let mut end = 0;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            positions.push(pseudo.unwrap_or(end));
            break;
        };
        match token {
            Token::WhiteSpace(_) => separated = true,
            Token::Comment(_) => {}
            Token::Comma => {
                positions.push(pseudo.unwrap_or(end));
                pseudo = None;
                colon = None;
                separated = false;
                end = parser.position().byte_index();
            }
            token => {
                if separated {
                    pseudo = None;
                    colon = None;
                    separated = false;
                }
                match token {
                    Token::Delim('>' | '+' | '~' | '|') => separated = true,
                    Token::Colon => {
                        if let Some(first) = colon {
                            pseudo.get_or_insert(first);
                        } else {
                            colon = Some(start);
                        }
                    }
                    Token::Ident(name) => {
                        if let Some(first) = colon
                            && is_legacy_pseudo_element(name)
                        {
                            pseudo.get_or_insert(first);
                        }
                        colon = None;
                    }
                    Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock => {
                        colon = None;
                        let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                            while inner.next_including_whitespace_and_comments().is_ok() {}
                            Ok(())
                        });
                    }
                    _ => colon = None,
                }
                end = parser.position().byte_index();
            }
        }
    }
    let mut output = LeanString::new();
    let mut cursor = 0;
    for position in positions {
        output.push_str(&selector[cursor..position]);
        output.push_str(condition);
        cursor = position;
    }
    output.push_str(&selector[cursor..]);
    output
}

fn is_legacy_pseudo_element(name: &str) -> bool {
    ["before", "after", "first-line", "first-letter"]
        .iter()
        .any(|pseudo| name.eq_ignore_ascii_case(pseudo))
}

/// Checks whether a selector can test real elements without pseudo-elements or
/// nested `:has()` functions. Set `inside_has` when the caller will insert this
/// selector into a `:has()` argument. Strings, attributes and escaped identifiers
/// do not contribute selector operations. This check does not validate the full
/// selector grammar.
pub fn selector_is_element_condition(selector: &str, inside_has: bool) -> bool {
    check_element_condition(&mut Parser::new(selector), inside_has)
}

/// Whether `&:is(<selector>)` is a valid selector that can match the element `&`
/// denotes.
///
/// The check fails when the selector contains a pseudo-element, because `:is()` cannot
/// contain one. It also fails when the final compound of a branch contains `:root`,
/// `:scope`, `:host`, `:host()`, or `:host-context()`. Those pseudo-classes select the
/// document root or a shadow host, so the branch cannot select the element that a
/// utility class styles. Strings, attributes and escaped identifiers do not contribute
/// selector operations. This check does not validate the full selector grammar.
pub fn selector_is_subject_condition(selector: &str) -> bool {
    selector_is_element_condition(selector, false)
        && !final_compound_selects_root(&mut Parser::new(selector))
}

/// Whether the final compound of a branch contains a pseudo-class for the document root
/// or a shadow host.
fn final_compound_selects_root(parser: &mut Parser<'_>) -> bool {
    let mut root = false;
    let mut colon = false;
    let mut separated = false;
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::WhiteSpace(_) => separated = true,
            Token::Comment(_) => {}
            Token::Comma => {
                if root {
                    return true;
                }
                colon = false;
                separated = false;
            }
            token => {
                if separated || matches!(token, Token::Delim('>' | '+' | '~')) {
                    root = false;
                    separated = false;
                }
                match token {
                    Token::Colon => {
                        colon = true;
                        continue;
                    }
                    Token::Ident(name) if colon => {
                        root |= ["root", "scope", "host"]
                            .iter()
                            .any(|pseudo| name.eq_ignore_ascii_case(pseudo));
                    }
                    Token::Function(name) => {
                        root |= colon
                            && ["host", "host-context"]
                                .iter()
                                .any(|pseudo| name.eq_ignore_ascii_case(pseudo));
                        let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                            while inner.next_including_whitespace_and_comments().is_ok() {}
                            Ok(())
                        });
                    }
                    Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock => {
                        let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                            while inner.next_including_whitespace_and_comments().is_ok() {}
                            Ok(())
                        });
                    }
                    _ => {}
                }
                colon = false;
            }
        }
    }
    root
}

fn check_element_condition(parser: &mut Parser<'_>, inside_has: bool) -> bool {
    let mut colons = 0;
    loop {
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return true;
        };
        match token {
            Token::Colon => {
                colons += 1;
                if colons == 2 {
                    return false;
                }
            }
            Token::Ident(name) => {
                if colons == 1 && is_legacy_pseudo_element(name) {
                    return false;
                }
                colons = 0;
            }
            Token::Function(name) => {
                let has = colons == 1 && name.eq_ignore_ascii_case("has");
                if has && inside_has {
                    return false;
                }
                colons = 0;
                let mut valid = true;
                let result: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    valid = check_element_condition(inner, inside_has || has);
                    Ok(())
                });
                if !valid || result.is_err() {
                    return false;
                }
            }
            Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                colons = 0;
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
            }
            Token::Comment(_) => {}
            _ => colons = 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_conditions_bind_to_each_final_originating_element() {
        for (selector, expected) in [
            (".a", ".a:focus"),
            (".a::before", ".a:focus::before"),
            (".a > .b::after:hover", ".a > .b:focus::after:hover"),
            (".a:before ", ".a:focus:before "),
            (".a::part(name)", ".a:focus::part(name)"),
            (".a::before, .b", ".a:focus::before, .b:focus"),
            (".a::before > .b", ".a::before > .b:focus"),
            (".a[data-text='::before']", ".a[data-text='::before']:focus"),
            (":is(.a, .b)", ":is(.a, .b):focus"),
            (".a:/**/:before", ".a:focus:/**/:before"),
        ] {
            assert_eq!(append_element_condition(selector, ":focus"), expected);
        }
    }

    #[test]
    fn element_conditions_check_nested_selector_functions_and_ignore_literal_text() {
        assert!(selector_is_element_condition(".r:has(.x):has(.y)", false));
        assert!(!selector_is_element_condition(".r:has(.x)", true));
        assert!(!selector_is_element_condition(
            ".r:has(:not(:has(.x)))",
            false
        ));
        assert!(!selector_is_element_condition(".r:is(.x, ::before)", false));
        assert!(!selector_is_element_condition(".r:before", false));
        assert!(selector_is_element_condition(
            r".r[data-text='::before :has(.x)']",
            true
        ));
        assert!(selector_is_element_condition(r".escaped\:\:before", true));
    }

    #[test]
    fn subject_conditions_reject_pseudo_elements_and_final_root_compounds() {
        for (selector, expected) in [
            (":hover", true),
            (":nth-child(3)", true),
            (".is-open", true),
            ("[data-x]", true),
            (":hover,:focus", true),
            (":root .dark", true),
            (":host .item", true),
            (":not(:root)", true),
            (r"[data-x=':root'], .\:root", true),
            ("::selection", false),
            ("::view-transition-old(*)", false),
            (".a:before", false),
            (":root", false),
            (":ROOT.dark", false),
            (":scope", false),
            (":host", false),
            (":host(.dark)", false),
            (":host-context(.dark) ", false),
            (".a > :root", false),
            (":hover, :root", false),
            (":root , .a", false),
        ] {
            assert_eq!(
                selector_is_subject_condition(selector),
                expected,
                "{selector}"
            );
        }
    }
}
