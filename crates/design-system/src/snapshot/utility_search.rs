//! Utility search metadata is derived once from an immutable snapshot.
//!
//! [`UtilitySearchIndex`] selects definitions by declared property or name prefix.
//! It preserves overloads so callers can search their separate value namespaces. It
//! also lists the functional families whose value and modifier set different
//! properties, such as `text-*` with a font size and a line height.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::LeanStr;

use stillwind_css::{CssRule, Declaration, find_css_functions};

use super::UtilityRegistry;
use crate::evaluation::placeholder_namespaces;

/// Lookup metadata for utility definitions in one snapshot.
#[derive(Debug, Default)]
pub struct UtilitySearchIndex {
    definitions: BTreeMap<LeanStr, Vec<UtilitySearchEntry>>,
    by_property: BTreeMap<LeanStr, BTreeSet<LeanStr>>,
    split_families: Vec<SplitFamily>,
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
                    let mut declarations = Vec::new();
                    nested_declarations(&definition.body, 0, &mut declarations);
                    let properties: BTreeSet<_> = declarations
                        .iter()
                        .map(|declaration| declaration.property.clone())
                        .collect();
                    for property in &properties {
                        index
                            .by_property
                            .entry(property.clone())
                            .or_default()
                            .insert(name.into());
                    }
                    let receiving = |placeholder: &str| -> BTreeSet<LeanStr> {
                        declarations
                            .iter()
                            .filter(|declaration| {
                                !find_css_functions(&declaration.css, &[placeholder], 0).is_empty()
                            })
                            .map(|declaration| declaration.property.clone())
                            .collect()
                    };
                    UtilitySearchEntry {
                        properties,
                        value_namespaces: placeholder_namespaces(&definition.body, "--value"),
                        value_properties: receiving("--value"),
                        modifier_properties: receiving("--modifier"),
                    }
                })
                .collect::<Vec<_>>();
            if let Some(root) = name.strip_suffix("-*") {
                for entry in &entries {
                    index.add_split_families(root, entry);
                }
            }
            index.definitions.insert(name.into(), entries);
        }
        index
    }

    /// Records the split families of `entry`, an overload of the family `root-*`. A
    /// standard property that receives only the value and a standard property that
    /// receives only the modifier form one split family.
    fn add_split_families(&mut self, root: &str, entry: &UtilitySearchEntry) {
        let only = |left: &BTreeSet<LeanStr>, right: &BTreeSet<LeanStr>| -> Vec<LeanStr> {
            left.difference(right)
                .filter(|property| !property.starts_with("--"))
                .cloned()
                .collect()
        };
        for value_property in only(&entry.value_properties, &entry.modifier_properties) {
            for modifier_property in only(&entry.modifier_properties, &entry.value_properties) {
                let family = SplitFamily {
                    root: root.into(),
                    value_property: value_property.clone(),
                    modifier_property,
                };
                if !self.split_families.contains(&family) {
                    self.split_families.push(family);
                }
            }
        }
    }

    /// Returns the functional families whose value sets one standard property and whose
    /// modifier sets another, in registration order. For example, `text-*` sets
    /// `font-size` from its value and `line-height` from its modifier.
    pub fn split_families(&self) -> &[SplitFamily] {
        &self.split_families
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
    /// Properties whose value contains a `--value()` placeholder.
    pub value_properties: BTreeSet<LeanStr>,
    /// Properties whose value contains a `--modifier()` placeholder.
    pub modifier_properties: BTreeSet<LeanStr>,
}

/// A functional family whose value sets one standard property and whose modifier sets
/// another, such as `text-*` with `font-size` and `line-height`. A candidate of the
/// family, such as `text-[1rem]/[1.5]`, can replace one utility for each property.
#[derive(Debug, Eq, PartialEq)]
pub struct SplitFamily {
    /// The family name without `-*`, such as `text`.
    pub root: LeanStr,
    /// The property that receives the value.
    pub value_property: LeanStr,
    /// The property that receives the modifier.
    pub modifier_property: LeanStr,
}

/// Returns the names of the properties that the CSS `body` declares, including custom
/// properties and properties in nested rules. Rules nested deeper than 64 levels add
/// nothing. When `body` cannot be parsed, the result is empty.
#[must_use]
pub fn declared_properties(body: &str) -> BTreeSet<LeanStr> {
    let mut declarations = Vec::new();
    nested_declarations(body, 0, &mut declarations);
    declarations
        .into_iter()
        .map(|declaration| declaration.property)
        .collect()
}

/// Appends to `output` the declarations of `body` and of its nested rules. `depth` is
/// the nesting level of `body`. Rules nested deeper than 64 levels, and CSS that cannot
/// be parsed, add nothing.
fn nested_declarations(body: &str, depth: usize, output: &mut Vec<Declaration>) {
    if depth > 64 {
        return;
    }
    if let Some(declarations) = Declaration::parse(body) {
        output.extend(declarations);
        return;
    }
    if let Ok(rules) = CssRule::parse(body) {
        for body in rules.iter().filter_map(|rule| rule.body.as_deref()) {
            nested_declarations(body, depth + 1, output);
        }
    }
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
                    implementation: false,
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
                    implementation: false,
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
                    implementation: false,
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

    #[test]
    fn split_families_pair_value_and_modifier_properties() {
        let utility = |body: &str| {
            (
                "text-*".into(),
                Arc::new(CustomUtility {
                    name: "text-*".into(),
                    body: body.into(),
                    position: None,
                    tw_sort: None,
                    role: UtilityRole::default(),
                    plugin: false,
                    implementation: false,
                }),
            )
        };
        let registry = UtilityRegistry::new([
            utility("font-size:--value([length]);line-height:--modifier([length]);"),
            utility("color:--value([color]);--tw-text-alpha:--modifier([percentage]);"),
        ]);
        let index = UtilitySearchIndex::new(&registry);
        assert_eq!(
            index.split_families(),
            [SplitFamily {
                root: "text".into(),
                value_property: "font-size".into(),
                modifier_property: "line-height".into(),
            }]
        );
    }
}
