//! The element that `&` refers to while a variant stack applies (the anchor).
//!
//! Variants apply from left to right, and the leftmost variant is the outermost one, as
//! in CSS Nesting. Before the first variant, the anchor is the generated class in
//! `style()` and the authored rule selector in `@apply`. A document-level style starts
//! without an anchor, so a variant must select an element before any variant refers to
//! `&`. [`document_target`] finds that variant in a resolved stack.

use stillwind_css::selector_uses_subject;

use super::application::VariantApplication;

/// Why a variant stack in document context does not select an element before it is
/// used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentTargetFailure {
    /// No variant selects an element in at least one alternative of the stack.
    NoTarget,
    /// The variant at this index refers to `&` in at least one alternative where no
    /// earlier variant selects an element.
    BeforeTarget(
        /// Index of the variant in the stack.
        usize,
    ),
}

/// Whether an element is selected before the first variant of a stack applies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VariantContext {
    /// `&` refers to an element from the start: the generated class of `style()` or
    /// the rule selector of `@apply`.
    #[default]
    Element,
    /// No element is selected at the start, as in a document-level style. A selector
    /// without `&` selects the element, and later variants apply relative to it.
    Document,
}

/// Returns the index of the variant that selects the element of a document-level style.
///
/// `applications` holds the resolved application of each variant in stack order.
/// [`VariantApplication::BareSelector`] and selectors without `&` select an element.
/// At-rule applications leave the selection unchanged. Other selectors, selector
/// conditions, and runtime conditions refer to `&`. When alternatives of a compound
/// application select the element at different variants, the largest index is
/// returned.
///
/// # Errors
///
/// Returns [`DocumentTargetFailure::BeforeTarget`] for the first variant that refers to
/// `&` before an element is selected in some alternative, and
/// [`DocumentTargetFailure::NoTarget`] when some alternative never selects an element.
pub fn document_target(
    applications: &[VariantApplication],
) -> Result<usize, DocumentTargetFailure> {
    let mut states = vec![None];
    for (index, application) in applications.iter().enumerate() {
        let mut next = Vec::new();
        for state in states {
            for state in advance(application, state, index)
                .ok_or(DocumentTargetFailure::BeforeTarget(index))?
            {
                if !next.contains(&state) {
                    next.push(state);
                }
            }
        }
        states = next;
    }
    states
        .into_iter()
        .try_fold(None, |largest: Option<usize>, state| {
            state.map(|state| Some(largest.map_or(state, |largest| largest.max(state))))
        })
        .flatten()
        .ok_or(DocumentTargetFailure::NoTarget)
}

/// Returns the possible selection states after `application` at `index`, starting from
/// `state`, the index of the variant that selected the element. Returns `None` when the
/// application refers to `&` while no element is selected.
fn advance(
    application: &VariantApplication,
    state: Option<usize>,
    index: usize,
) -> Option<Vec<Option<usize>>> {
    let needs = || state.map(|_| vec![state]);
    match application {
        VariantApplication::Compound(children) => {
            let mut states = Vec::new();
            for child in children {
                states.extend(advance(child, state, index)?);
            }
            Some(states)
        }
        VariantApplication::Sequence(children) => {
            children.iter().try_fold(vec![state], |states, child| {
                let mut next = Vec::new();
                for state in states {
                    next.extend(advance(child, state, index)?);
                }
                Some(next)
            })
        }
        VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => Some(vec![state]),
        VariantApplication::BareSelector { .. } => Some(vec![state.or(Some(index))]),
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            if selector_uses_subject(selector) {
                needs()
            } else {
                Some(vec![state.or(Some(index))])
            }
        }
        VariantApplication::SelectorSuffix(_)
        | VariantApplication::SelectorCondition(_)
        | VariantApplication::Conditional(_) => needs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(selector: &str, top_level: &str) -> VariantApplication {
        VariantApplication::BareSelector {
            selector: selector.into(),
            top_level: top_level.into(),
        }
    }

    #[test]
    fn the_first_selector_without_nesting_selector_selects_the_element() {
        let hover = VariantApplication::SelectorTemplate("&:hover".into());
        let media = VariantApplication::Media("print".into());
        let host = bare(":host", ":host");
        assert_eq!(document_target(std::slice::from_ref(&host)), Ok(0));
        assert_eq!(
            document_target(&[media.clone(), host.clone(), hover.clone()]),
            Ok(1)
        );
        assert_eq!(
            document_target(&[bare("&:is(:hover)", ":hover"), bare(":root", ":root")]),
            Ok(0)
        );
        assert_eq!(
            document_target(&[VariantApplication::SelectorTemplate(":root".into())]),
            Ok(0)
        );
        assert_eq!(
            document_target(std::slice::from_ref(&media)),
            Err(DocumentTargetFailure::NoTarget)
        );
        assert_eq!(document_target(&[]), Err(DocumentTargetFailure::NoTarget));
        assert_eq!(
            document_target(&[media, hover.clone(), host]),
            Err(DocumentTargetFailure::BeforeTarget(1))
        );
        for application in [
            VariantApplication::SelectorCondition(":not(:hover)".into()),
            VariantApplication::SelectorSuffix(":hover"),
            VariantApplication::Conditional("--on".into()),
            hover,
        ] {
            assert_eq!(
                document_target(std::slice::from_ref(&application)),
                Err(DocumentTargetFailure::BeforeTarget(0)),
                "{application:?}"
            );
        }
    }

    #[test]
    fn every_alternative_must_select_an_element() {
        let host = bare(":host", ":host");
        let alternatives = VariantApplication::Compound(vec![
            host.clone(),
            VariantApplication::Media("print".into()),
        ]);
        assert_eq!(
            document_target(std::slice::from_ref(&alternatives)),
            Err(DocumentTargetFailure::NoTarget)
        );
        assert_eq!(document_target(&[alternatives, host.clone()]), Ok(1));
        let sequence = VariantApplication::Sequence(vec![
            VariantApplication::Supports("(display:grid)".into()),
            host,
            VariantApplication::SelectorTemplate("&:hover".into()),
        ]);
        assert_eq!(document_target(&[sequence]), Ok(0));
    }
}
