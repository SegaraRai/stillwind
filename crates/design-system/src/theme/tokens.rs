//! Resolves namespace resets and token replacements once during theme construction.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanString};

use crate::{ThemeNamespace, ThemeToken, ThemeTokenKind};

#[derive(Debug)]
pub(super) struct TokenLookup {
    pub(super) active: BTreeMap<LeanStr, usize>,
    pub(super) named: BTreeMap<ThemeNamespace, BTreeMap<LeanStr, usize>>,
    pub(super) scales: BTreeMap<ThemeNamespace, usize>,
}

impl TokenLookup {
    pub(super) fn new(tokens: &[Arc<ThemeToken>]) -> Self {
        let mut values = BTreeMap::new();
        let mut clear_all = false;
        for (index, token) in tokens.iter().enumerate() {
            if token.name.value == "--*" && token.value.value == "initial" {
                values.clear();
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
                values.retain(|name: &LeanStr, _| {
                    !name.starts_with(format_args!("--{namespace}-").to_lean_string().as_str())
                });
                continue;
            }
            if clear_all || token.name.value.starts_with("--") {
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
