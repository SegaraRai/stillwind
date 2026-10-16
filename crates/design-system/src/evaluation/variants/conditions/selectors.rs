//! Selector conditions on the styled element, and whether a variant keeps the element
//! that the rule styles.

use lean_string::ToLeanStr;

use super::super::VariantApplication;
use super::atoms::{Condition, Test};

/// Whether `application` only adds conditions to the styled element, so that applying
/// it twice has the same effect as applying it once.
///
/// Media, container, and support conditions and runtime conditions qualify. A selector
/// qualifies when it is `&` followed by a compound selector without pseudo-elements.
/// Selectors that select another element, the starting style, and variants that
/// declare properties do not qualify.
pub(super) fn is_filter(application: &VariantApplication) -> bool {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().all(is_filter)
        }
        VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::Conditional(_) => true,
        VariantApplication::SelectorSuffix(suffix) => subject_compound(suffix).is_some(),
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. }
        | VariantApplication::SelectorCondition(selector)
        | VariantApplication::SelectorMedia { selector, .. } => selector
            .strip_prefix('&')
            .and_then(subject_compound)
            .is_some(),
        VariantApplication::SelectorRule { .. }
        | VariantApplication::SelectorMediaRule { .. }
        | VariantApplication::StartingStyle => false,
    }
}

/// Whether `application` contains a selector that is not a filter of the styled
/// element according to [`is_filter`].
pub(super) fn moves_subject(application: &VariantApplication) -> bool {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().any(moves_subject)
        }
        VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::Conditional(_)
        | VariantApplication::StartingStyle => false,
        VariantApplication::SelectorSuffix(suffix) => subject_compound(suffix).is_none(),
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. }
        | VariantApplication::SelectorCondition(selector)
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => selector
            .strip_prefix('&')
            .and_then(subject_compound)
            .is_none(),
    }
}

/// Splits the text after `&` into its simple selectors, or returns `None` when the text
/// is empty, contains a combinator, a pseudo-element, a selector list, a type selector,
/// or another `&`.
fn subject_compound(compound: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0_usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in compound.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, character) {
            (_, '\\') => escaped = true,
            (Some(open), character) if character == open => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, ':') if depth == 0 && compound[..index].ends_with(':') => return None,
            (None, ':' | '[' | '.' | '#') if depth == 0 => {
                if index > start {
                    parts.push(&compound[start..index]);
                    start = index;
                }
                depth += usize::from(character == '[');
            }
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth = depth.checked_sub(1)?,
            (None, '&' | ',') if depth == 0 => return None,
            (None, character)
                if depth == 0 && (character.is_whitespace() || "> +~|".contains(character)) =>
            {
                return None;
            }
            _ => {}
        }
    }
    if depth != 0 || quote.is_some() || start == compound.len() {
        return None;
    }
    parts.push(&compound[start..]);
    parts
        .iter()
        .all(|part| part.starts_with([':', '[', '.', '#']))
        .then_some(parts)
}

/// The conditions that `selector` adds to the styled element, or none when it is not a
/// compound selector on `&`. `:not(x)` with a single selector `x` becomes the negation
/// of `x`.
pub(super) fn selector_conditions(selector: &str, variant: usize) -> Vec<Condition> {
    let Some(parts) = selector.strip_prefix('&').and_then(subject_compound) else {
        return Vec::new();
    };
    parts
        .into_iter()
        .map(|part| {
            let negated = part
                .strip_prefix(":not(")
                .and_then(|inner| inner.strip_suffix(')'))
                .filter(|inner| subject_compound(inner).is_some_and(|parts| parts.len() == 1));
            Condition {
                variant,
                test: Test::Opaque {
                    key: format_args!("& {}", negated.unwrap_or(part)).to_lean_str(),
                    negated: negated.is_some(),
                },
                display: format_args!("&{part}").to_lean_str(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_contribute_conditions_only_on_the_styled_element() {
        let template = |selector: &str| {
            selector_conditions(selector, 0)
                .into_iter()
                .map(|condition| condition.test)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            template("&:hover:not(:focus)"),
            [
                Test::Opaque {
                    key: "& :hover".into(),
                    negated: false,
                },
                Test::Opaque {
                    key: "& :focus".into(),
                    negated: true,
                },
            ]
        );
        assert_eq!(template("& > *"), []);
        assert_eq!(template("&::before"), []);
        assert_eq!(template(":where(.dark) &"), []);
        assert_eq!(template("&:is(.a, .b)").len(), 1);
        assert_eq!(template("&[data-state=\"a b\"]").len(), 1);
        assert!(is_filter(&VariantApplication::SelectorTemplate(
            "&:is(:where(.group):hover *)".into()
        )));
        assert!(moves_subject(&VariantApplication::SelectorTemplate(
            ":is(& > *)".into()
        )));
    }
}
