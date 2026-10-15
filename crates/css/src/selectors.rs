//! Expansion of nesting selector templates at rule construction time.

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanString;

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
