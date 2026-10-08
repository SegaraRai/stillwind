//! Resolved immutable lookup views, including selected built-ins and overloads.
//!
//! Definitions are stored as shared values grouped by name. Grouping is not a merge: a
//! name with several definitions keeps them in insertion order, which callers rely on
//! when they pick the last matching branch, and the fingerprint of a group changes if
//! that order changes.
//!
//! Iteration order is the name order of an ordered map, so callers that need authored
//! order must read the underlying definition vectors instead. These merged views serve
//! lexical iteration and fingerprints. Candidate matching uses each layer's index and
//! its precedence; both views share the same immutable definition values.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::CustomPropertyDefinition;
use stillwind_source::Fingerprint;

use crate::{CustomUtility, CustomVariant, ThemeToken};

/// Immutable name-to-definitions lookup produced by one snapshot.
///
/// The type parameter is the definition value; every alias below names one concrete
/// registry so callers do not have to repeat it.
#[derive(Clone, Debug)]
pub struct DefinitionRegistry<T> {
    /// Definitions grouped by name in lexical key order; each vector preserves
    /// insertion order, including overloads.
    entries: BTreeMap<LeanStr, Vec<Arc<T>>>,
}

impl<T> DefinitionRegistry<T> {
    /// Groups definitions by name, keeping insertion order inside each group.
    ///
    /// The grouping is the resolved view of built-ins, framework imports, theme CSS,
    /// and authored definitions in the order the snapshot supplied them; this
    /// constructor must not reorder or deduplicate.
    pub(crate) fn new(entries: impl IntoIterator<Item = (LeanStr, Arc<T>)>) -> Self {
        let mut grouped: BTreeMap<LeanStr, Vec<Arc<T>>> = BTreeMap::new();
        for (name, value) in entries {
            grouped.entry(name).or_default().push(value);
        }
        Self { entries: grouped }
    }

    /// Returns every definition registered under `name`, in insertion order, or `None`
    /// when the name is unknown.
    pub fn get(&self, name: &str) -> Option<&[Arc<T>]> {
        self.entries.get(name).map(Vec::as_slice)
    }

    /// Iterates groups in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Arc<T>])> {
        self.entries
            .iter()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
    }

    /// Iterates names in ascending order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(LeanStr::as_str)
    }

    /// Returns names starting with `prefix`, in lexical order.
    pub fn names_with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> {
        self.entries
            .range::<str, _>((
                std::ops::Bound::Included(prefix),
                std::ops::Bound::Unbounded,
            ))
            .take_while(move |(name, _)| name.starts_with(prefix))
            .map(|(name, _)| name.as_str())
    }
}

/// Active theme tokens, keyed by custom-property name.
pub type ThemeRegistry = DefinitionRegistry<ThemeToken>;

/// Selected built-in and authored utilities, keyed by authored name.
pub type UtilityRegistry = DefinitionRegistry<CustomUtility>;

/// Selected built-in and authored variants, keyed by authored name.
pub type VariantRegistry = DefinitionRegistry<CustomVariant>;

/// `@stillwind-property` definitions, keyed by property name.
pub type PropertyRegistry = DefinitionRegistry<CustomPropertyDefinition>;

impl<T: std::fmt::Debug> DefinitionRegistry<T> {
    /// One content fingerprint per group, derived from the ordered definitions so an
    /// overload reorder or edit invalidates dependents.
    pub(crate) fn fingerprints(&self) -> BTreeMap<String, Fingerprint> {
        self.entries
            .iter()
            .map(|(name, values)| (name.to_string(), Fingerprint::of(format!("{values:?}"))))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overload_order_survives_grouping_and_affects_fingerprints() {
        let registry = DefinitionRegistry::new([
            ("x".into(), Arc::new(1)),
            ("x".into(), Arc::new(2)),
            ("a".into(), Arc::new(3)),
        ]);
        assert_eq!(
            registry
                .get("x")
                .unwrap()
                .iter()
                .map(|v| **v)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(registry.names().collect::<Vec<_>>(), vec!["a", "x"]);
        assert!(registry.get("missing").is_none());
        let reversed =
            DefinitionRegistry::new([("x".into(), Arc::new(2)), ("x".into(), Arc::new(1))]);
        assert_ne!(registry.fingerprints()["x"], reversed.fingerprints()["x"]);
    }
}
