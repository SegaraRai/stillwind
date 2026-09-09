use super::validation::ValueType;

pub(crate) fn is_css_wide_keyword(value: &str) -> bool {
    matches!(
        value.trim(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer"
    )
}

/// A `--value([…])` domain. These names are CSS value definitions, not CSS
/// property names. Complex built-in grammars receive a named value domain
/// only when they cannot be expressed as a composition of standard types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDomain {
    Type(ValueType),
}

impl ValueDomain {
    #[must_use]
    pub fn from_hint(hint: &str) -> Option<Self> {
        ValueType::from_hint(hint).map(Self::Type)
    }

    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        match self {
            Self::Type(value_type) => value_type.accepts(value),
        }
    }

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
