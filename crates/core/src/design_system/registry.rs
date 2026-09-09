//! Resolved immutable lookup views, including selected built-ins and overloads.

use std::{collections::BTreeMap, sync::Arc};

use stillwind_css::{CustomPropertyDefinition, CustomUtility, CustomVariant, ThemeToken};

use crate::Fingerprint;

#[derive(Clone, Debug)]
pub struct DefinitionRegistry<T> {
    entries: BTreeMap<String, Vec<Arc<T>>>,
}

impl<T> DefinitionRegistry<T> {
    pub(crate) fn new(entries: impl IntoIterator<Item = (String, Arc<T>)>) -> Self {
        let mut grouped: BTreeMap<String, Vec<Arc<T>>> = BTreeMap::new();
        for (name, value) in entries {
            grouped.entry(name).or_default().push(value);
        }
        Self { entries: grouped }
    }
    pub fn get(&self, name: &str) -> Option<&[Arc<T>]> {
        self.entries.get(name).map(Vec::as_slice)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Arc<T>])> {
        self.entries
            .iter()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

pub type ThemeRegistry = DefinitionRegistry<ThemeToken>;

pub type UtilityRegistry = DefinitionRegistry<CustomUtility>;

pub type VariantRegistry = DefinitionRegistry<CustomVariant>;

pub type PropertyRegistry = DefinitionRegistry<CustomPropertyDefinition>;

impl<T: std::fmt::Debug> DefinitionRegistry<T> {
    pub(crate) fn fingerprints(&self) -> BTreeMap<String, Fingerprint> {
        self.entries
            .iter()
            .map(|(name, values)| (name.clone(), Fingerprint::of(format!("{values:?}"))))
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
