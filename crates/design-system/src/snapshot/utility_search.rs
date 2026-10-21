//! Utility search metadata is derived once from an immutable snapshot.
//!
//! [`UtilitySearchIndex`] selects definitions by declared property or name prefix.
//! It preserves overloads so callers can search their separate value namespaces.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::LeanStr;

use stillwind_css::{CssRule, Declaration, find_css_functions};

use super::UtilityRegistry;

/// Lookup metadata for utility definitions in one snapshot.
#[derive(Debug, Default)]
pub struct UtilitySearchIndex {
    definitions: BTreeMap<LeanStr, Vec<UtilitySearchEntry>>,
    by_property: BTreeMap<LeanStr, BTreeSet<LeanStr>>,
}

impl UtilitySearchIndex {
    /// Indexes every definition of `utilities` by name and by the properties that its
    /// body declares.
    pub(super) fn new(utilities: &UtilityRegistry) -> Self {
        let mut index = Self::default();
        for (name, definitions) in utilities.iter() {
            let entries = definitions
                .iter()
                .map(|definition| {
                    let properties = declared_properties(&definition.body, 0);
                    for property in &properties {
                        index
                            .by_property
                            .entry(property.clone())
                            .or_default()
                            .insert(name.into());
                    }
                    let value_namespaces = find_css_functions(&definition.body, &["--value"], 0)
                        .into_iter()
                        .flat_map(|found| definition.body[found.arguments].split(','))
                        .filter_map(|part| part.trim().strip_suffix('*'))
                        .filter(|part| part.starts_with("--"))
                        .map(LeanStr::from)
                        .collect();
                    UtilitySearchEntry {
                        properties,
                        value_namespaces,
                    }
                })
                .collect();
            index.definitions.insert(name.into(), entries);
        }
        index
    }

    /// Returns definition names that declare `property`, in lexical order.
    /// Nested declaration blocks are included, with a maximum depth of 64.
    pub fn names_for_property(&self, property: &str) -> impl Iterator<Item = &str> {
        self.by_property
            .get(property)
            .into_iter()
            .flatten()
            .map(LeanStr::as_str)
    }

    /// Returns names starting with `prefix` and their overloads in lexical name order.
    /// Each overload keeps its registration order within its name group.
    pub fn definitions_with_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a [UtilitySearchEntry])> {
        self.definitions
            .range::<str, _>((
                std::ops::Bound::Included(prefix),
                std::ops::Bound::Unbounded,
            ))
            .take_while(move |(name, _)| name.starts_with(prefix))
            .map(|(name, entries)| (name.as_str(), entries.as_slice()))
    }
}

/// Properties and theme value domains declared by one utility overload.
#[derive(Debug)]
pub struct UtilitySearchEntry {
    /// Authored declaration property names, including names inside nested blocks.
    pub properties: BTreeSet<LeanStr>,
    /// Custom-property prefixes accepted by `--value`, without the trailing `*`.
    /// Entries retain their order and may repeat.
    pub value_namespaces: Vec<LeanStr>,
}

/// Returns the property names that `body` declares, including those in nested rules.
///
/// `depth` is the nesting level of `body`. Rules nested deeper than 64 levels
/// contribute nothing, and a body that cannot be parsed has no properties.
fn declared_properties(body: &str, depth: usize) -> BTreeSet<LeanStr> {
    if depth > 64 {
        return BTreeSet::new();
    }
    if let Some(declarations) = Declaration::parse(body) {
        return declarations
            .into_iter()
            .map(|declaration| declaration.property)
            .collect();
    }
    CssRule::parse(body).map_or_else(
        |_| BTreeSet::new(),
        |rules| {
            rules
                .into_iter()
                .filter_map(|rule| rule.body)
                .flat_map(|body| declared_properties(&body, depth + 1))
                .collect()
        },
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{CustomUtility, UtilityRole};

    #[test]
    fn search_keeps_nested_properties_overloads_and_prefix_boundaries() {
        let registry = UtilityRegistry::new([
            (
                "tone-*".into(),
                Arc::new(CustomUtility {
                    name: "tone-*".into(),
                    body: "&:hover { border-width:--value(--spacing-*, [length]); }".into(),
                    position: None,
                    tw_sort: None,
                    role: UtilityRole::default(),
                    plugin: false,
                }),
            ),
            (
                "tone-*".into(),
                Arc::new(CustomUtility {
                    name: "tone-*".into(),
                    body: "border-color:--value(--color-*, [color]);".into(),
                    position: None,
                    tw_sort: None,
                    role: UtilityRole::default(),
                    plugin: false,
                }),
            ),
            (
                "toned".into(),
                Arc::new(CustomUtility {
                    name: "toned".into(),
                    body: "color:red;".into(),
                    position: None,
                    tw_sort: None,
                    role: UtilityRole::default(),
                    plugin: false,
                }),
            ),
        ]);
        let index = UtilitySearchIndex::new(&registry);
        assert_eq!(
            index.names_for_property("border-width").collect::<Vec<_>>(),
            ["tone-*"]
        );
        let groups = index.definitions_with_prefix("tone-").collect::<Vec<_>>();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1[0].value_namespaces, ["--spacing-"]);
        assert_eq!(groups[0].1[1].value_namespaces, ["--color-"]);
        assert!(index.names_for_property("missing").next().is_none());
    }
}
