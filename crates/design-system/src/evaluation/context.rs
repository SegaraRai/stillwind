//! Local records of the values selected while evaluating one declaration or variant.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ops::Deref;

use stillwind_css::inspect_css_value;

use crate::ThemeIndex;

/// Borrows immutable definitions and optionally records direct source selections.
pub(crate) struct EvaluationContext<'a> {
    theme: &'a ThemeIndex,
    selections: Option<RefCell<ValueSelections>>,
}

impl<'a> EvaluationContext<'a> {
    pub(crate) fn untracked(theme: &'a ThemeIndex) -> Self {
        Self {
            theme,
            selections: None,
        }
    }

    pub(crate) fn tracked(theme: &'a ThemeIndex) -> Self {
        Self {
            theme,
            selections: Some(RefCell::new(ValueSelections::default())),
        }
    }

    pub(crate) fn record_token(&self, name: &str) {
        if let Some(selections) = &self.selections {
            selections.borrow_mut().tokens.insert(name.to_owned());
        }
    }

    pub(crate) fn record_value_tokens(&self, value: &str) {
        if self.selections.is_some() {
            for name in inspect_css_value(value).1 {
                self.record_token(&name);
            }
        }
    }

    pub(crate) fn record_variable_token(&self, name: &str) {
        if self.selections.is_some() {
            self.record_value_tokens(&format!("var({name})"));
        }
    }

    pub(crate) fn record_arbitrary_value(&self, value: &str, color: bool) {
        if let Some(selections) = &self.selections {
            selections
                .borrow_mut()
                .values
                .push(ArbitraryValueSelection {
                    value: value.to_owned(),
                    color,
                });
        }
    }

    /// Removes the current selections and transfers ownership to the caller.
    pub(crate) fn take_selections(&self) -> ValueSelections {
        self.selections
            .as_ref()
            .map(RefCell::take)
            .unwrap_or_default()
    }
}

impl Deref for EvaluationContext<'_> {
    type Target = ThemeIndex;

    fn deref(&self) -> &Self::Target {
        self.theme
    }
}

/// Direct source selections before expansion and declaration formatting.
#[derive(Default)]
pub(crate) struct ValueSelections {
    pub(crate) tokens: BTreeSet<String>,
    pub(crate) values: Vec<ArbitraryValueSelection>,
}

/// An accepted arbitrary capture before theme expansion and declaration formatting.
pub(crate) struct ArbitraryValueSelection {
    pub(crate) value: String,
    pub(crate) color: bool,
}
