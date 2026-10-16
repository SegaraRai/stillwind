//! Resolved variant applications and the queries that inspect them.
//!
//! [`VariantApplication`] is the data that resolution produces: a selector, a
//! condition, declarations, or a composition of those. The compiler decides how to
//! combine it with a utility. [`VariantObstacle`] names the part of an application that a
//! negating or relationship variant cannot express. [`document_scoped_selector`] and
//! [`repeated_subject_selector`] find selectors that hosts report.

use std::fmt;

use lean_string::{LeanStr, LeanString};

use stillwind_css::{
    SelectorTarget, selector_is_element_condition, selector_repeats_subject, selector_target,
    selector_uses_subject,
};
use stillwind_syntax::{Variant, VariantKind};

use super::resolve::{
    parse_nested_variant, resolve_named_variant_with, resolve_registered_variant_candidate,
    resolve_variant, split_typed_capture,
};
use crate::{BuiltinVariantValueType, ThemeIndex};

/// A variant's selector, at-rule, declaration, or computed-value effect.
/// Resolution retains runtime conditions as data; rendering chooses their CSS strategy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    /// Tests custom-property presence or equality at computed-value time.
    Conditional(
        /// Canonical [`RuntimeCondition`] text: a presence test or `--name:value`
        /// comparison, optionally prefixed by one `!`.
        /// Refers to the styled element or pseudo-element, never a ref ancestor.
        LeanStr,
    ),
    /// Several applications, one per block of a compound body, in authored order.
    Compound(Vec<VariantApplication>),
    /// Applications composed in order within one branch, retaining nested wrappers.
    Sequence(Vec<VariantApplication>),
    /// A fixed selector suffix appended to the utility's own selector.
    SelectorSuffix(&'static str),
    /// A resolved selector template containing `&` for the utility selector.
    SelectorTemplate(LeanStr),
    /// Adds a compound selector condition to the final element selected by a rule.
    /// The renderer inserts it before that element's pseudo-elements, if present.
    SelectorCondition(LeanStr),
    /// A selector template that also declares properties of its own.
    SelectorRule {
        /// Selector template for the wrapped utility.
        selector: LeanStr,
        /// Declarations the variant contributes.
        declarations: Vec<VariantDeclaration>,
    },
    /// A selector template inside a media condition.
    SelectorMedia {
        /// Selector template for the wrapped utility.
        selector: LeanStr,
        /// Media condition the rule is nested under.
        condition: LeanStr,
    },
    /// A selector template inside a media condition that also declares properties of
    /// its own.
    SelectorMediaRule {
        /// Selector template for the wrapped utility.
        selector: LeanStr,
        /// Declarations the variant contributes.
        declarations: Vec<VariantDeclaration>,
        /// Media condition the rule is nested under.
        condition: LeanStr,
    },
    /// A media condition that wraps the utility without changing its selector.
    Media(LeanStr),
    /// A container condition that wraps the utility without changing its selector.
    Container(LeanStr),
    /// A support condition that wraps the utility without changing its selector.
    Supports(
        /// Complete `@supports` prelude, such as `(display:grid)` or
        /// `not (display:grid)`.
        LeanStr,
    ),
    /// The starting-style at-rule, which the emitter applies around the rule.
    StartingStyle,
}

impl VariantApplication {
    /// Whether resolving this application requires the current element's selector.
    /// External container identities do not themselves require a subject selector.
    pub fn uses_subject(&self) -> bool {
        match self {
            Self::Conditional(_) | Self::SelectorSuffix(_) | Self::SelectorCondition(_) => true,
            Self::Compound(children) | Self::Sequence(children) => {
                children.iter().any(Self::uses_subject)
            }
            Self::SelectorTemplate(selector)
            | Self::SelectorRule { selector, .. }
            | Self::SelectorMedia { selector, .. }
            | Self::SelectorMediaRule { selector, .. } => selector_uses_subject(selector),
            Self::Media(_) | Self::Container(_) | Self::Supports(_) | Self::StartingStyle => false,
        }
    }
}

/// One declaration carried by a variant rather than by the utility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDeclaration {
    /// Property name the variant sets.
    pub property: LeanStr,
    /// Value of that property.
    pub value: LeanStr,
}

/// What an inner variant does that its negating or relationship variant cannot express.
///
/// [`fmt::Display`] completes the clause "`not-` cannot negate `x`, which ...".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariantObstacle {
    /// A selector selects a pseudo-element, which `:not()`, `:is()`, `:where()`, and
    /// `:has()` cannot test.
    PseudoElement,
    /// A selector styles descendants of the element instead of the element itself.
    Descendants,
    /// A selector styles a sibling or another element related by a combinator.
    OtherElement,
    /// A selector does not refer to the styled element, such as `:root`.
    NoSubject,
    /// A media, container, support, or starting-style condition, which a selector
    /// cannot contain.
    AtRule,
    /// The `@starting-style` rule, which has no negated form.
    StartingStyle,
    /// The variant declares properties of its own.
    Declarations,
    /// A custom-property test evaluated on the styled element.
    RuntimeCondition,
    /// The negation needs more alternatives than the compiler emits.
    TooManyAlternatives,
    /// Any other application that cannot be combined.
    Unsupported,
}

impl VariantObstacle {
    /// Classifies a selector template that a negating or relationship variant cannot
    /// use as a condition on the styled element.
    ///
    /// Returns `None` when the selector starts with `&` and selects only the styled
    /// element.
    pub(super) fn of_selector(selector: &str) -> Option<Self> {
        if !selector_is_element_condition(selector, false) {
            Some(Self::PseudoElement)
        } else if !selector_uses_subject(selector) {
            Some(Self::NoSubject)
        } else if selector_target(selector) != SelectorTarget::Attachment
            || !selector.trim_start().starts_with('&')
        {
            Some(if selector.contains(['+', '~']) {
                Self::OtherElement
            } else {
                Self::Descendants
            })
        } else {
            None
        }
    }
}

impl fmt::Display for VariantObstacle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PseudoElement => "selects a pseudo-element",
            Self::Descendants => "styles descendants",
            Self::OtherElement => "styles another element",
            Self::NoSubject => "does not select the styled element",
            Self::AtRule => "is an at-rule condition",
            Self::StartingStyle => "applies `@starting-style`",
            Self::Declarations => "declares properties",
            Self::RuntimeCondition => "tests a custom property",
            Self::TooManyAlternatives => "has too many alternatives",
            Self::Unsupported => "cannot be expressed as a condition on the styled element",
        })
    }
}

/// Returns the selector of the first selector-bearing application that does not
/// reference the subject element, if one exists.
///
/// A rule whose selector never mentions its subject is live wherever the stylesheet
/// loads, so a host can reject such input where the rule must stay scoped to the
/// element that produced it. Conditions, runtime conditions, and selectors that keep
/// the subject are not reported.
#[must_use]
pub fn document_scoped_selector(application: &VariantApplication) -> Option<&str> {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().find_map(document_scoped_selector)
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            (!selector_uses_subject(selector)).then_some(selector.as_str())
        }
        VariantApplication::Conditional(_)
        | VariantApplication::SelectorCondition(_)
        | VariantApplication::SelectorSuffix(_)
        | VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => None,
    }
}

/// Returns a selector of `variant` that uses `&` more than once in one selector.
///
/// `&` stands for the generated utility class. A selector that repeats it, such as
/// `& + &`, matches elements by the identity of generated classes, which depends on the
/// compiler's class naming rather than on an element relationship. The check covers the
/// selectors of the resolved application and, for a `[variant]` definition such as
/// `group-*` or `not-*`, the selectors of its inner variant. The inner variant is
/// checked first because `not-*` replaces its `&` with `*`. Returns `None` for an
/// unresolved variant.
#[must_use]
pub fn repeated_subject_selector(variant: &Variant, theme: &ThemeIndex) -> Option<LeanString> {
    let (application, inner) = match &variant.kind {
        VariantKind::Arbitrary(_) => (resolve_variant(variant, theme).ok()?, None),
        VariantKind::Named(value) => {
            resolve_named_variant_with(value, theme, |custom, capture, concrete| {
                let application =
                    resolve_registered_variant_candidate(custom, capture, concrete, 0, theme)
                        .ok()?;
                let inner = capture
                    .filter(|_| custom.value_type == Some(BuiltinVariantValueType::Variant))
                    .map(|capture| parse_nested_variant(split_typed_capture(custom, capture).0));
                Some((application, inner))
            })?
        }
    };
    inner
        .and_then(|inner| repeated_subject_selector(&inner, theme))
        .or_else(|| repeated_application_selector(&application).map(LeanString::from))
}

/// Returns the first selector of `application` that repeats `&` in one selector.
fn repeated_application_selector(application: &VariantApplication) -> Option<&str> {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().find_map(repeated_application_selector)
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::SelectorCondition(selector)
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            selector_repeats_subject(selector).then_some(selector.as_str())
        }
        VariantApplication::Conditional(_)
        | VariantApplication::SelectorSuffix(_)
        | VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => None,
    }
}
