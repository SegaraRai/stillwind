//! Writes equivalent selectors in one spelling, so that the CSS of two candidates can
//! be compared. The compiler output does not use these spellings.

use std::fmt::Write;
use std::ops::Range;

use cssparser::{Parser, Token};

use stillwind_css::{
    CssFunctionMatch, CssRule, Declaration, find_css_functions, matching_css_delimiter,
    selector_list_branches,
};

/// Returns `css` with each selector written in one spelling by
/// `subject_with_specificity`. For example, `[data-x]&` becomes `&[data-x]`, and
/// `&:is([data-x])` becomes `&[data-x]`.
///
/// When `preserve_specificity` is false, the method may also remove `:where()` around
/// an ancestor selector, which changes specificity. At-rule conditions and declarations
/// keep their text. `depth` is the nesting level of `css`. CSS that cannot be parsed,
/// and rules nested deeper than 64 levels, stay as written.
pub(super) fn selector_signatures(css: &str, preserve_specificity: bool, depth: usize) -> String {
    if depth > 64 {
        return css.into();
    }
    CssRule::parse(css).map_or_else(
        |_| css.into(),
        |rules| {
            rules.into_iter().fold(String::new(), |mut output, rule| {
                let Some(body) = &rule.body else {
                    output.push_str(&rule.prelude);
                    return output;
                };
                let prelude = if rule.prelude.trim_start().starts_with('@') {
                    rule.prelude.to_string()
                } else {
                    selector_list_branches(&rule.prelude)
                        .into_iter()
                        .map(|branch| subject_with_specificity(branch, preserve_specificity))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let body = if Declaration::parse(body).is_some() {
                    body.to_string()
                } else {
                    selector_signatures(body, preserve_specificity, depth + 1)
                };
                let _ = write!(output, "{prelude}{{{body}}}");
                output
            })
        },
    )
}

/// Returns the byte range of the pseudo-class `call` in `selector`, including its
/// leading `:`, such as `:is(.a)`. Returns `None` when no `:` comes directly before
/// `call`, or when `call` has no closing parenthesis.
fn pseudo_class_range(selector: &str, call: &CssFunctionMatch) -> Option<Range<usize>> {
    let start = call
        .start
        .checked_sub(1)
        .filter(|start| selector.as_bytes().get(*start) == Some(&b':'))?;
    Some(start..call.end?)
}

/// Returns one selector, without commas, in the spelling that comparison uses.
///
/// The method makes these changes:
///
/// - `:not(:nth-child(odd))` becomes `:nth-child(even)`, and the reverse.
/// - An `:is()` with one selector is removed where the result selects the same
///   elements. The method repeats this at most 32 times for nested calls.
/// - When `preserve_specificity` is false, a `:where()` at the start, followed by `&`,
///   is also removed.
/// - In a compound selector, `&` moves to the start, and attribute selectors follow it.
fn subject_with_specificity(selector: &str, preserve_specificity: bool) -> String {
    let mut selector = selector.trim().to_string();
    for call in find_css_functions(&selector.clone(), &["not"], 0)
        .into_iter()
        .rev()
    {
        let Some(Range { start, end }) = pseudo_class_range(&selector, &call) else {
            continue;
        };
        let replacement = match selector[call.arguments].trim() {
            ":nth-child(odd)" => Some(":nth-child(even)"),
            ":nth-child(even)" => Some(":nth-child(odd)"),
            _ => None,
        };
        if let Some(replacement) = replacement {
            selector.replace_range(start..end, replacement);
        }
    }
    for _ in 0..32 {
        let mut changed = false;
        let calls = find_css_functions(&selector, &["is", "where"], 0);
        for call in calls.into_iter().rev() {
            let Some(Range { start, end }) = pseudo_class_range(&selector, &call) else {
                continue;
            };
            let inner = selector[call.arguments.clone()].trim();
            // A selector that starts with a combinator is relative. `:is()` does not
            // accept it, so removing `:is()` would change what the selector means.
            if selector_list_branches(inner).len() != 1 || inner.starts_with(['>', '+', '~']) {
                continue;
            }
            let before = &selector[..start];
            let after = &selector[end..];
            if call.name_index == 1
                && (preserve_specificity
                    || !before.is_empty()
                    || !after.trim_start().starts_with('&'))
            {
                continue;
            }
            let replacement = if before.is_empty() {
                let mut inner = inner.to_string();
                if after.starts_with(['[', ':', '.', '#']) && inner.ends_with('*') {
                    inner.pop();
                }
                Some(inner)
            } else if (before == "&"
                || before.ends_with(":has(")
                || before.ends_with(":not(")
                || (before.starts_with('&') && compound(before) && !inner.contains('&')))
                && compound(inner)
            {
                Some(inner.into())
            } else {
                None
            };
            if let Some(replacement) = replacement {
                selector.replace_range(start..end, &replacement);
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    if compound(&selector) {
        let mut parser = Parser::new(&selector);
        let mut found = None;
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            if matches!(token, Token::Delim('&')) {
                found = Some(parser.position().byte_index() - 1);
            }
        }
        drop(parser);
        if let Some(index) = found.filter(|index| *index > 0) {
            selector.remove(index);
            selector.insert(0, '&');
        }
    }
    attributes_first(selector)
}

/// Moves the attribute selectors of `selector` directly after its leading `&`, in their
/// original order. For example, `&:hover[data-x]` becomes `&[data-x]:hover`. The method
/// changes only a compound selector that starts with `&` and has no pseudo-element. It
/// returns other selectors unchanged.
fn attributes_first(mut selector: String) -> String {
    if !selector.starts_with('&') || selector.contains("::") || !compound(&selector) {
        return selector;
    }
    let mut ranges = Vec::new();
    let mut parser = Parser::new(&selector);
    while let Ok(token) = parser.next() {
        if matches!(token, Token::SquareBracketBlock) {
            let start = parser.position().byte_index() - 1;
            if let Ok(end) = matching_css_delimiter(&selector, start, '[', ']') {
                ranges.push(start..end + 1);
            }
        }
    }
    drop(parser);
    let attributes: String = ranges
        .iter()
        .map(|range| &selector[range.clone()])
        .collect();
    for range in ranges.into_iter().rev() {
        selector.replace_range(range, "");
    }
    selector.insert_str(1, &attributes);
    selector
}

/// Checks whether `selector` is one compound selector, so it contains no whitespace,
/// comma, or combinator.
fn compound(selector: &str) -> bool {
    let mut parser = Parser::new(selector);
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        if matches!(
            token,
            Token::WhiteSpace(_) | Token::Comma | Token::Delim('>' | '+' | '~' | '|')
        ) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(selector: &str) -> String {
        subject_with_specificity(selector, true)
    }

    #[test]
    fn preserves_descendants_and_selector_lists() {
        assert_eq!(subject("[data-visible]&"), "&[data-visible]");
        assert_eq!(subject(":is(& > *)[data-visible]"), "& > [data-visible]");
        assert_eq!(subject("&:is([data-visible])"), "&[data-visible]");
        assert_eq!(subject("[data-visible] &"), "[data-visible] &");
        assert_eq!(subject(":is(&,.other):focus"), ":is(&,.other):focus");
    }
}
