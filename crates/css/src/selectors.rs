//! Expansion of nesting selector templates at rule construction time.

use cssparser::{ParseError, Parser, Token};

/// Whether a selector contains a nesting token referring to its current subject.
/// Quoted, escaped, commented, and attribute-value ampersands are literal text.
pub fn selector_uses_subject(selector: &str) -> bool {
    let mut positions = Vec::new();
    collect(&mut Parser::new(selector), &mut positions);
    !positions.is_empty()
}

/// Emits a selector template with CSS nesting tokens bound to `parent`.
/// Strings, comments, escaped identifiers and attribute values remain literal.
pub fn instantiate_selector(template: &str, parent: &str) -> String {
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

/// Adds `condition` to the final element compound of each selector branch.
/// The caller must supply a valid selector and a compound selector fragment such
/// as `:is(.scope *)`. The fragment is inserted before the final compound's first
/// pseudo-element, so the condition tests its originating element. Selector lists,
/// whitespace, comments and literal text are preserved.
pub fn append_element_condition(selector: &str, condition: &str) -> String {
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
    let mut output = String::new();
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
}
