//! Local records of the values selected while evaluating one declaration or variant.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ops::Deref;

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::inspect_css_value;

use crate::ThemeIndex;

/// Borrows immutable definitions and optionally records direct source selections.
pub(in crate::evaluation) struct EvaluationContext<'a> {
    theme: &'a ThemeIndex,
    selections: Option<RefCell<ValueSelections>>,
}

impl<'a> EvaluationContext<'a> {
    pub(in crate::evaluation) fn untracked(theme: &'a ThemeIndex) -> Self {
        Self {
            theme,
            selections: None,
        }
    }

    pub(in crate::evaluation) fn tracked(theme: &'a ThemeIndex) -> Self {
        Self {
            theme,
            selections: Some(RefCell::new(ValueSelections::default())),
        }
    }

    pub(in crate::evaluation) fn record_token(&self, name: &str) {
        if let Some(selections) = &self.selections {
            selections.borrow_mut().tokens.insert(name.to_lean_str());
        }
    }

    pub(in crate::evaluation) fn record_value_tokens(&self, value: &str) {
        if self.selections.is_some() {
            for name in inspect_css_value(value).1 {
                self.record_token(&name);
            }
        }
    }

    pub(in crate::evaluation) fn record_variable_token(&self, name: &str) {
        if self.selections.is_some() {
            self.record_value_tokens(&format_args!("var({name})").to_lean_string());
        }
    }

    pub(in crate::evaluation) fn record_arbitrary_value(&self, value: &str, color: bool) {
        if let Some(selections) = &self.selections {
            selections
                .borrow_mut()
                .values
                .push(ArbitraryValueSelection {
                    value: value.to_lean_str(),
                    color,
                });
        }
    }

    /// Removes the current selections and transfers ownership to the caller.
    pub(in crate::evaluation) fn take_selections(&self) -> ValueSelections {
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
pub(in crate::evaluation) struct ValueSelections {
    pub(in crate::evaluation) tokens: BTreeSet<LeanStr>,
    pub(in crate::evaluation) values: Vec<ArbitraryValueSelection>,
}

/// An accepted arbitrary capture before theme expansion and declaration formatting.
pub(in crate::evaluation) struct ArbitraryValueSelection {
    pub(in crate::evaluation) value: LeanStr,
    pub(in crate::evaluation) color: bool,
}
