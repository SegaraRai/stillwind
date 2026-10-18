//! Resolves namespace resets and token replacements once during theme construction.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanString};

use crate::{ThemeNamespace, ThemeToken, ThemeTokenKind};

/// Positions of the active tokens in a theme's token list.
#[derive(Debug)]
pub(super) struct TokenLookup {
    /// Active definition of each custom-property name.
    pub(super) active: BTreeMap<LeanStr, usize>,
    /// Active named tokens of each namespace, by name within the namespace.
    pub(super) named: BTreeMap<ThemeNamespace, BTreeMap<LeanStr, usize>>,
    /// Active scale token of each namespace, such as `--spacing`.
    pub(super) scales: BTreeMap<ThemeNamespace, usize>,
}

impl TokenLookup {
    /// Applies `tokens` in authored order. A definition replaces an earlier definition
    /// of the same name unless the earlier one has higher precedence. `--*: initial`
    /// and `--namespace-*: initial` remove the earlier definitions of the same or lower
    /// precedence, and after `--*: initial` only custom properties are kept.
    pub(super) fn new(tokens: &[Arc<ThemeToken>]) -> Self {
        let mut values = BTreeMap::new();
        let mut clear_all = false;
        for (index, token) in tokens.iter().enumerate() {
            if token.name.value == "--*" && token.value.value == "initial" {
                values.retain(|_, previous: &mut usize| {
                    tokens[*previous].precedence > token.precedence
                });
                clear_all = true;
                continue;
            }
            if let Some(namespace) = token
                .name
                .value
                .strip_prefix("--")
                .and_then(|value| value.strip_suffix("-*"))
                && token.value.value == "initial"
            {
                values.retain(|name: &LeanStr, previous: &mut usize| {
                    !name.starts_with(format_args!("--{namespace}-").to_lean_string().as_str())
                        || tokens[*previous].precedence > token.precedence
                });
                continue;
            }
            if clear_all || token.name.value.starts_with("--") {
                if values
                    .get(&token.name.value)
                    .is_some_and(|previous: &usize| tokens[*previous].precedence > token.precedence)
                {
                    continue;
                }
                values.insert(token.name.value.clone(), index);
            }
        }

        let mut named: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
        let mut scales = BTreeMap::new();
        for index in values.values() {
            match &tokens[*index].kind {
                ThemeTokenKind::Token { namespace, name } => {
                    named
                        .entry(*namespace)
                        .or_default()
                        .insert(name.clone(), *index);
                }
                ThemeTokenKind::Scale(namespace) => {
                    scales.insert(*namespace, *index);
                }
                ThemeTokenKind::Custom => {}
            }
        }
        Self {
            active: values,
            named,
            scales,
        }
    }
}
