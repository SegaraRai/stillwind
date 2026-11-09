//! The nesting selector `&`: finding, counting, inserting, and replacing it, and
//! recognizing wrappers that test its element.

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanString;

use crate::functions::find_css_functions;
use crate::lists::split_top_level;
use crate::parser::{consume_css_block, validate_css_syntax};

use super::element::selector_is_element_condition;
use super::list::{pseudo_element_start, selector_list_branches};

/// Returns the element condition of a complete `:is()` wrapper whose branches end
/// in one nesting selector, such as `:is(.group:hover &)`. Each branch must have
/// exactly one subject, at its end after whitespace or a combinator, and cannot contain
/// pseudo-elements. The subject becomes `*`, so callers can test the condition without
/// moving their styled element. Returns [`None`] for every other selector form.
pub fn wrapped_subject_condition(selector: &str) -> Option<LeanString> {
    is_wrapped_subject(selector).then(|| subject_as_universal(selector))
}

/// Emits a variant selector template with CSS nesting tokens bound to `parent`.
///
/// A wrapper that [`wrapped_subject_condition`] accepts, such as
/// `:is(.group:hover &)`, binds each branch of `parent` separately. A pseudo-element
/// at the end of a branch moves outside the wrapper, so `.x::before` gives
/// `:is(.group:hover .x)::before`. `:is()` cannot contain a pseudo-element, so
/// `:is(.group:hover .x::before)` would never match. Other templates are bound as
/// [`instantiate_selector`] binds them.
pub fn instantiate_variant_selector(template: &str, parent: &str) -> LeanString {
    if !is_wrapped_subject(template) {
        return instantiate_selector(template, parent);
    }
    let branches = selector_list_branches(parent)
        .into_iter()
        .map(|branch| match pseudo_element_start(branch) {
            Some(start) if !branch[..start].trim().is_empty() => {
                let (element, pseudo_element) = branch.split_at(start);
                // `.x ::before` selects the pseudo-element of every descendant.
                let element = if element.ends_with(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, '>' | '+' | '~')
                }) {
                    format!("{element}*")
                } else {
                    element.to_owned()
                };
                format!(
                    "{}{pseudo_element}",
                    instantiate_selector(template, &element)
                )
            }
            _ => instantiate_selector(template, branch).to_string(),
        })
        .collect::<Vec<_>>();
    branches.join(",").into()
}

fn is_wrapped_subject(selector: &str) -> bool {
    if !selector_is_element_condition(selector, false) {
        return false;
    }
    let Some(wrapper) = find_css_functions(selector, &["is"], 0).into_iter().next() else {
        return false;
    };
    if wrapper.start != 1 || wrapper.end != Some(selector.len()) || !selector.starts_with(':') {
        return false;
    }
    split_top_level(&selector[wrapper.arguments], |token| {
        matches!(token, Token::Comma)
    })
    .iter()
    .all(|branch| ends_with_separate_subject(branch))
}

/// Whether `branch` ends with its only nesting selector, after whitespace or a
/// combinator and a non-empty prefix. Comments do not separate compounds, so
/// `.a/**/&` puts the subject in the same compound as `.a`.
fn ends_with_separate_subject(branch: &str) -> bool {
    let mut parser = Parser::new(branch);
    let mut separated = false;
    let mut subject = None;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        match token {
            Token::WhiteSpace(_) => {
                separated = true;
                continue;
            }
            Token::Comment(_) => continue,
            Token::Delim('>' | '+' | '~') => {
                separated = true;
                subject = None;
                continue;
            }
            Token::Delim('&') => subject = separated.then_some(start),
            _ => subject = None,
        }
        separated = false;
    }
    subject.is_some_and(|start| {
        let prefix = &branch[..start];
        !prefix.trim().is_empty() && !selector_uses_subject(prefix)
    })
}

/// Splits the simple selectors following a nesting selector into authored slices.
/// Returns None for an empty suffix, combinators, pseudo-elements, type selectors,
/// selector lists, malformed groups, or another nesting selector.
pub fn subject_compound_parts(compound: &str) -> Option<Vec<&str>> {
    validate_css_syntax(compound).ok()?;
    let mut parser = Parser::new(compound);
    let mut parts = Vec::new();
    loop {
        let start = parser.position().byte_index();
        let token = match parser.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(_) => break,
        };
        match token {
            Token::Comment(_) => {
                continue;
            }
            Token::Colon => match parser.next_including_whitespace_and_comments().ok()? {
                Token::Ident(_) => {}
                Token::Function(_) => {
                    consume_css_block(&mut parser).ok()?;
                }
                _ => return None,
            },
            Token::SquareBracketBlock => {
                consume_css_block(&mut parser).ok()?;
            }
            Token::Delim('.') => {
                if !matches!(
                    parser.next_including_whitespace_and_comments(),
                    Ok(Token::Ident(_))
                ) {
                    return None;
                }
            }
            Token::IDHash(_) => {}
            _ => return None,
        }
        parts.push(&compound[start..parser.position().byte_index()]);
    }
    (!parts.is_empty()).then_some(parts)
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
            Token::Delim('&') => {
                current += 1;
            }
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
/// and do not count as `&`. [`implicit_nesting_positions`] returns the insertion
/// points.
pub fn absolutize_nested_selector(selector: &str) -> LeanString {
    let mut output = LeanString::new();
    let mut cursor = 0;
    for position in implicit_nesting_positions(selector) {
        output.push_str(&selector[cursor..position]);
        output.push_str("& ");
        cursor = position;
    }
    output.push_str(&selector[cursor..]);
    output
}

/// Returns the byte offset of the first token of each branch of `selector` that
/// [`absolutize_nested_selector`] prefixes with `& `, in increasing order.
///
/// The result is empty when every branch contains `&` and no branch starts with a
/// combinator.
pub fn implicit_nesting_positions(selector: &str) -> Vec<usize> {
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
            Token::Delim('&') => {
                uses_subject = true;
            }
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
    insertions
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
            Token::Delim('&') => {
                positions.push(start);
            }
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
    fn wrapped_subjects_follow_whitespace_or_a_combinator() {
        for selector in [
            ":is(.group:hover &)",
            ":is(.peer:checked~&)",
            ":is(.a>&, .b
&)",
        ] {
            assert!(wrapped_subject_condition(selector).is_some(), "{selector}");
        }
        for selector in [
            ":is(.a&)",
            ":is(.a/**/&)",
            ":is(& .a)",
            ":is(& &)",
            ":is(.a &, .b)",
        ] {
            assert!(wrapped_subject_condition(selector).is_none(), "{selector}");
        }
    }

    #[test]
    fn variant_wrappers_bind_branches_and_keep_pseudo_elements_outside() {
        let template = ":is(.group:hover &)";
        for (parent, expected) in [
            (".x::before", ":is(.group:hover .x)::before"),
            (
                ".x *::selection, .x::selection",
                ":is(.group:hover .x *)::selection,:is(.group:hover .x)::selection",
            ),
            (".x ::marker", ":is(.group:hover .x *)::marker"),
            (".x:hover", ":is(.group:hover .x:hover)"),
        ] {
            assert_eq!(
                instantiate_variant_selector(template, parent),
                expected,
                "{parent}"
            );
        }
        assert_eq!(
            instantiate_variant_selector("& > *", ".x::before"),
            ".x::before > *"
        );
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
