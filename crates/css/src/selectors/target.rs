//! Classification of the element that each selector branch styles.

use cssparser::{ParseError, Parser, Token};

/// Which element each branch identifies, using a pseudo-element's originating element.
/// The nesting selector `&` denotes the attachment element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorTarget {
    /// Every branch identifies the attachment element.
    Attachment,
    /// At least one branch provably identifies another element.
    Other,
    /// No branch proves another target, but attachment cannot be established.
    Unknown,
}

/// Classifies each selector branch by its originating element.
/// Arguments of `:has()` and `:not()` do not identify declaration targets.
/// Unknown functional selectors remain conservative.
pub fn selector_target(selector: &str) -> SelectorTarget {
    classify_target(&mut Parser::new(selector))
}

fn classify_target(parser: &mut Parser<'_>) -> SelectorTarget {
    let mut attachment = false;
    let mut uncertain = false;
    let mut separated = false;
    let mut colon = false;
    let mut other = false;
    let mut branches = Vec::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::WhiteSpace(_) => separated = true,
            Token::Comment(_) => {}
            Token::Comma => {
                branches.push(if other || !attachment && !uncertain {
                    SelectorTarget::Other
                } else if attachment {
                    SelectorTarget::Attachment
                } else {
                    SelectorTarget::Unknown
                });
                attachment = false;
                uncertain = false;
                separated = false;
                colon = false;
                other = false;
            }
            token => {
                if separated || matches!(token, Token::Delim('>' | '+' | '~' | '|')) {
                    attachment = false;
                    uncertain = false;
                    colon = false;
                    other = false;
                    separated = false;
                }
                match token {
                    Token::Delim('&') => {
                        attachment = true;
                        colon = false;
                    }
                    Token::Colon => {
                        colon = true;
                    }
                    Token::Function(name) => {
                        let list = colon
                            && (name.eq_ignore_ascii_case("is")
                                || name.eq_ignore_ascii_case("where"));
                        let predicate =
                            name.eq_ignore_ascii_case("has") || name.eq_ignore_ascii_case("not");
                        let mut target = SelectorTarget::Unknown;
                        let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                            if list {
                                target = classify_target(inner);
                            } else {
                                while inner.next_including_whitespace_and_comments().is_ok() {}
                            }
                            Ok(())
                        });
                        if list && !attachment {
                            match target {
                                SelectorTarget::Attachment => attachment = true,
                                SelectorTarget::Other => other = true,
                                SelectorTarget::Unknown => uncertain = true,
                            }
                        } else if !attachment && !predicate {
                            uncertain = true;
                        }
                        colon = false;
                    }
                    Token::SquareBracketBlock => {
                        let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                            while inner.next_including_whitespace_and_comments().is_ok() {}
                            Ok(())
                        });
                        colon = false;
                    }
                    _ => colon = false,
                }
            }
        }
    }
    branches.push(if other || !attachment && !uncertain {
        SelectorTarget::Other
    } else if attachment {
        SelectorTarget::Attachment
    } else {
        SelectorTarget::Unknown
    });
    if branches.contains(&SelectorTarget::Other) {
        SelectorTarget::Other
    } else if branches.contains(&SelectorTarget::Unknown) {
        SelectorTarget::Unknown
    } else {
        SelectorTarget::Attachment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_targets_preserve_predicates_and_skip_unknown_functions() {
        for (selector, expected) in [
            (".parent &", SelectorTarget::Attachment),
            ("&:has(>img):not(.disabled)", SelectorTarget::Attachment),
            (":where(&:hover, &:focus)", SelectorTarget::Attachment),
            ("& > span", SelectorTarget::Other),
            ("&:before", SelectorTarget::Attachment),
            ("&::before", SelectorTarget::Attachment),
            ("&::after:hover", SelectorTarget::Attachment),
            ("&::part(label)", SelectorTarget::Attachment),
            ("&::slotted(span)", SelectorTarget::Attachment),
            ("&::before, &::after", SelectorTarget::Attachment),
            (
                ":where(&::before, &:focus::after)",
                SelectorTarget::Attachment,
            ),
            ("& > span::before", SelectorTarget::Other),
            ("&::before, & > span::after", SelectorTarget::Other),
            (":is(&,span)::before", SelectorTarget::Other),
            (":is(&,span)", SelectorTarget::Other),
            (":future-selector(&)", SelectorTarget::Unknown),
            ("&[data-text='& > span']", SelectorTarget::Attachment),
        ] {
            assert_eq!(selector_target(selector), expected, "{selector}");
        }
    }
}
