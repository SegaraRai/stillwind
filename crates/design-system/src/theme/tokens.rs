//! Resolves namespace resets and token replacements once during theme construction.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use crate::{ThemeNamespaces, ThemeToken};

/// Positions of the active tokens in a theme's token list.
#[derive(Debug)]
pub(super) struct TokenLookup {
    /// Active definition of each custom-property name.
    pub(super) active: BTreeMap<LeanStr, usize>,
    /// Active named tokens of each declared namespace, by namespace name and then by
    /// key within the namespace.
    pub(super) named: BTreeMap<LeanStr, BTreeMap<LeanStr, usize>>,
    /// Active scale token of each declared namespace, by namespace name.
    pub(super) scales: BTreeMap<LeanStr, usize>,
}

impl TokenLookup {
    /// Applies `tokens` in authored order. A definition replaces an earlier definition
    /// of the same name unless the earlier one has higher precedence. `--*: initial`
    /// and `--namespace-*: initial` remove the earlier definitions of the same or lower
    /// precedence, and after `--*: initial` only custom properties are kept.
    ///
    /// `namespaces` classifies each active token: a namespace's bare name is its scale,
    /// and a longer name is a named token of the longest matching namespace.
    pub(super) fn new(tokens: &[Arc<ThemeToken>], namespaces: &ThemeNamespaces) -> Self {
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
            match namespaces.classify(&tokens[*index].name.value) {
                Some((namespace, Some(key))) => {
                    named
                        .entry(namespace.name.clone())
                        .or_default()
                        .insert(key.to_lean_str(), *index);
                }
                Some((namespace, None)) => {
                    scales.insert(namespace.name.clone(), *index);
                }
                None => {}
            }
        }
        Self {
            active: values,
            named,
            scales,
        }
    }
}
