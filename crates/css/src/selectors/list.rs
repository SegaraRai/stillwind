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

/// Whether every branch of `selector` puts a combinator after a known pseudo-element or
/// contains an `:is()` or `:where()` whose branches all do so.
///
/// This is a check for selectors produced by nesting, not full grammar validation.
/// Unknown pseudo-elements remain available to framework transforms. Attributes,
/// strings, comments, and non-selector function arguments are not inspected.
pub fn selector_cannot_match(selector: &str) -> bool {
    selector_list_branches(selector)
        .into_iter()
        .all(branch_cannot_match)
}

fn branch_cannot_match(selector: &str) -> bool {
    let mut parser = Parser::new(selector);
    let mut pseudo_element = false;
    let mut separated = false;
    let mut colons = 0;
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::Comment(_) => continue,
            Token::WhiteSpace(_) => {
                separated = true;
                continue;
            }
            _ if pseudo_element
                && (separated || matches!(token, Token::Delim('>' | '+' | '~' | '|'))) =>
            {
                return true;
            }
            Token::Colon => {
                colons += 1;
                separated = false;
                continue;
            }
            Token::Ident(name) => {
                pseudo_element |= (colons == 2 && known_pseudo_element(name))
                    || (colons == 1 && legacy_pseudo_element(name));
            }
            Token::Function(name) => {
                pseudo_element |= colons == 2 && known_pseudo_element(name);
                let selector_arguments = colons == 1
                    && (name.eq_ignore_ascii_case("is") || name.eq_ignore_ascii_case("where"));
                let mut dead = false;
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    let start = inner.position();
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    if selector_arguments {
                        dead = selector_cannot_match(inner.slice_from(start));
                    }
                    Ok(())
                });
                if dead {
                    return true;
                }
            }
            Token::SquareBracketBlock | Token::ParenthesisBlock | Token::CurlyBracketBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
            }
            _ => {}
        }
        colons = 0;
        separated = false;
    }
    false
}

fn legacy_pseudo_element(name: &str) -> bool {
    ["before", "after", "first-line", "first-letter"]
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
}

fn known_pseudo_element(name: &str) -> bool {
    legacy_pseudo_element(name)
        || [
            "selection",
            "marker",
            "backdrop",
            "placeholder",
            "file-selector-button",
            "details-content",
            "target-text",
            "search-text",
            "cue",
            "cue-region",
            "part",
            "slotted",
            "highlight",
            "picker",
            "picker-icon",
            "checkmark",
            "grammar-error",
            "spelling-error",
            "view-transition",
            "view-transition-group",
            "view-transition-image-pair",
            "view-transition-old",
            "view-transition-new",
            "-moz-selection",
            "-moz-placeholder",
            "-ms-input-placeholder",
            "-ms-browse",
            "-webkit-input-placeholder",
            "-webkit-file-upload-button",
            "-webkit-details-marker",
            "-webkit-backdrop",
            "-webkit-scrollbar",
            "-webkit-scrollbar-button",
            "-webkit-scrollbar-track",
            "-webkit-scrollbar-track-piece",
            "-webkit-scrollbar-thumb",
            "-webkit-scrollbar-corner",
            "-webkit-resizer",
        ]
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
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
            Err(_) => {
                break;
            }
        }
    }
    branches.push(selector[start..].trim());
    branches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_rejects_combinators_after_known_pseudo_elements() {
        for selector in [
            ".a::selection > *",
            ".a::selection .child",
            ".a::backdrop + p",
            ".a::-webkit-details-marker ~ p",
            ".a:before > p",
            ".a::part(label) > p",
            ".a::SELECTION:hover > p",
            ".a::selection/* > */ > p",
            ":where(.a::selection > *, .a::marker + p)",
            ":is(:where(.a::selection > *))",
        ] {
            assert!(selector_cannot_match(selector), "{selector}");
        }
        for selector in [
            ".a > ::selection",
            ".a::selection:hover",
            ".a::before::marker",
            ".a::ng-deep > p",
            ".a::v-deep > p",
            ".a::future-element > p",
            ".a[data-x='::selection > *'] > p",
            ".a\\:\\:selection > p",
            ".a::selection/* > */:hover",
            ":is(.a::selection > *, .fallback)",
            ".a:not(.b::selection > *)",
            ".a::selection > *, .fallback",
        ] {
            assert!(!selector_cannot_match(selector), "{selector}");
        }
    }

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
