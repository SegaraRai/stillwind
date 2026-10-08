//! The domain value a `--value([…])` hint denotes.
//!
//! A domain is deliberately one layer above [`ValueType`]: it is the type the registry
//! stores and compares, and it decides both whether a concrete value is accepted and
//! whether a narrower hint may annotate a broader domain. Hint names are CSS value
//! definitions, never CSS property names, so a `property:padding` style hint resolves
//! to nothing.

use super::validation::ValueType;

/// True for the five CSS-wide keywords, which every property accepts before its own
/// grammar is considered.
///
/// The comparison trims whitespace and is case-sensitive, so only the canonical
/// lowercase spelling is recognized.
pub(super) fn is_css_wide_keyword(value: &str) -> bool {
    matches!(
        value.trim(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer"
    )
}

/// A `--value([…])` domain. These names are CSS value definitions, not CSS property
/// names. Complex built-in grammars receive a named value domain only when they cannot
/// be expressed as a composition of standard types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDomain {
    /// One of the standard CSS value types.
    Type(ValueType),
}

impl ValueDomain {
    /// Resolves the public hint spelling, or `None` when no value type has that name.
    #[must_use]
    pub fn from_hint(hint: &str) -> Option<Self> {
        ValueType::from_hint(hint).map(Self::Type)
    }

    /// Validates one decoded value against this domain.
    ///
    /// Accepts CSS-wide keywords and `var()`-bearing values regardless of the type; see
    /// [`ValueType::accepts`] for why a custom-property reference is not a static type
    /// error.
    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        match self {
            Self::Type(value_type) => value_type.accepts(value),
        }
    }

    /// True when `hint` names a narrower value type this domain also accepts, for
    /// example `<length>` within `<length-percentage>`.
    ///
    /// [`ValueType::Any`] accepts every hint, and an unknown hint is never narrower.
    #[must_use]
    pub fn accepts_hint(&self, hint: &str) -> bool {
        match self {
            Self::Type(expected) => {
                ValueType::from_hint(hint).is_some_and(|actual| expected.accepts_hint(actual))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_and_hint_boundaries_do_not_admit_unknown_domains() {
        for keyword in ["initial", "inherit", "unset", "revert", "revert-layer"] {
            assert!(is_css_wide_keyword(keyword));
        }
        assert!(!is_css_wide_keyword("initially"));
        assert_eq!(ValueDomain::from_hint("unknown"), None);
        assert!(ValueDomain::from_hint("property:padding").is_none());
        let domain = ValueDomain::from_hint("length-percentage").unwrap();
        assert!(domain.accepts("2px"));
        assert!(domain.accepts_hint("percentage"));
        assert!(!domain.accepts_hint("time"));
        assert!(!domain.accepts_hint("unknown"));
    }
}
