//! Lists the proposals for a group, and checks how CSS properties of candidates and
//! proposals relate.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssRule, Declaration, properties_interfere, property_covers};
use stillwind_syntax::{Utility, parse_candidate, print_arbitrary_value};

use super::super::index::rank;
use super::compare::{physical, physical_properties};
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Returns the declarations that the utility `value` sets on the element, without
    /// custom properties. Returns `None` when `value` does not resolve, or when its CSS
    /// is not one rule for the element itself, such as CSS with a nested selector.
    pub(super) fn candidate_declarations(&self, value: &str) -> Option<Vec<Declaration>> {
        let preview = self.context.canonical_preview(value).ok()??;
        let rules = CssRule::parse(&preview.css).ok()?;
        if rules.len() != 1 || rules[0].prelude.trim() != "&" {
            return None;
        }
        Some(
            Declaration::parse(rules[0].body.as_deref()?)?
                .into_iter()
                .filter(|declaration| !declaration.property.starts_with("--"))
                .collect(),
        )
    }

    /// Returns the proposals for the candidates `values`, sorted by `rank`.
    ///
    /// `inputs` must contain the declarations of each candidate in `values`, in the
    /// same order. `None` marks a candidate without usable declarations, and such a
    /// candidate adds no proposals.
    ///
    /// A utility family adds proposals when it declares a property of two or more
    /// candidates. A family with a fixed name, such as `sr-only`, proposes that name. A
    /// functional family `root-*` proposes `root-` followed by the value that the
    /// candidate's own family captures, and `root-[value]` for each declared value.
    /// For a family whose value and modifier set different properties, such as
    /// `text-*` with `font-size` and `line-height`, each pair of values of those
    /// properties also proposes `root-[value]/[modifier]`.
    pub(super) fn collapse_proposals(
        &self,
        values: &[LeanStr],
        inputs: &[Option<Vec<Declaration>>],
    ) -> Vec<LeanStr> {
        let mut family_uses = BTreeMap::new();
        for declarations in inputs.iter().flatten() {
            let families: BTreeSet<_> = declarations
                .iter()
                .flat_map(|declaration| self.property_families(&declaration.property))
                .collect();
            for family in families {
                *family_uses.entry(family).or_insert(0) += 1;
            }
        }
        let mut proposals = BTreeSet::new();
        let mut property_values = BTreeMap::<&str, BTreeSet<String>>::new();
        for (raw, declarations) in values.iter().zip(inputs) {
            let Some(declarations) = declarations else {
                continue;
            };
            let capture = self.capture(raw);
            for declaration in declarations {
                let Some((_, value)) = declaration.css.split_once(':') else {
                    continue;
                };
                let value = value.trim().trim_end_matches(';');
                property_values
                    .entry(declaration.property.as_str())
                    .or_default()
                    .insert(value.to_string());
                for family in self.property_families(&declaration.property) {
                    if family_uses.get(family).is_none_or(|uses| *uses < 2) {
                        continue;
                    }
                    if let Some(root) = family.strip_suffix("-*") {
                        if let Some(capture) = &capture {
                            proposals.insert(format!("{root}-{capture}").into());
                        }
                        proposals
                            .insert(format!("{root}-[{}]", print_arbitrary_value(value)).into());
                    } else if !family.contains('*') {
                        proposals.insert(family.to_lean_str());
                    }
                }
            }
        }
        for family in self.context.design_system.utility_search().split_families() {
            let (Some(values), Some(modifiers)) = (
                property_values.get(family.value_property.as_str()),
                property_values.get(family.modifier_property.as_str()),
            ) else {
                continue;
            };
            for value in values {
                for modifier in modifiers {
                    proposals.insert(
                        format!(
                            "{}-[{}]/[{}]",
                            family.root,
                            print_arbitrary_value(value),
                            print_arbitrary_value(modifier)
                        )
                        .into(),
                    );
                }
            }
        }
        let mut proposals: Vec<_> = proposals.into_iter().collect();
        proposals.sort_by(|left: &LeanStr, right| rank(left, right));
        proposals
    }

    /// Checks whether a proposal that sets the declarations `properties` may replace
    /// two or more of `inputs`.
    ///
    /// Returns true when two or more inputs set a property that interferes with a
    /// property of the proposal, and when each property of the proposal covers, or is
    /// covered by, a property of some input. A true result does not prove that the
    /// replacement keeps the CSS, so the caller must still compare the CSS.
    pub(super) fn proposal_fits(
        &self,
        properties: &[Declaration],
        inputs: &[Option<Vec<Declaration>>],
    ) -> bool {
        if inputs
            .iter()
            .flatten()
            .filter(|source| {
                source.iter().any(|declaration| {
                    properties
                        .iter()
                        .any(|target| properties_interfere(&target.property, &declaration.property))
                })
            })
            .take(2)
            .count()
            < 2
        {
            return false;
        }
        properties.iter().all(|target| {
            inputs.iter().flatten().flatten().any(|source| {
                self.covers(&target.property, &source.property)
                    || self.covers(&source.property, &target.property)
            })
        })
    }

    /// Returns the value that a registered family captures from the named utility
    /// `raw`, such as `4` from `w-4`. When several families match, the family with the
    /// longest root is used. Returns `None` for an arbitrary property, or when no
    /// family matches.
    fn capture(&self, raw: &str) -> Option<LeanStr> {
        let parsed = parse_candidate(raw)?;
        let Utility::Named { name, .. } = parsed.utility else {
            return None;
        };
        name.match_indices('-').rev().find_map(|(index, _)| {
            self.context
                .design_system
                .utilities()
                .get(&format!("{}-*", &name[..index]))?;
            Some(name[index + 1..].to_lean_str())
        })
    }

    /// Checks whether a declaration of the property `target` also sets the property
    /// `source`, as `padding` sets `padding-top`. A property covers itself. When the
    /// `logicalToPhysical` option is enabled, the check also compares physical names,
    /// so `margin-inline` covers `margin-left`.
    pub(super) fn covers(&self, target: &str, source: &str) -> bool {
        if target == source {
            return true;
        }
        if property_covers(target, source) {
            return true;
        }
        self.canonical_options().logical_to_physical
            && physical_properties(target).iter().any(|target| {
                target == &physical(source) || property_covers(target, &physical(source))
            })
    }

    /// Returns the utility names that declare `property`, or a shorthand that covers
    /// it. Names include family names such as `p-*`.
    ///
    /// The method considers only shorthands whose name consists of parts of `property`,
    /// such as `padding` for `padding-top`. When the `logicalToPhysical` option is
    /// enabled, it also considers the logical names of physical sides. A property name
    /// with more than six parts returns no names.
    pub(super) fn property_families(&self, property: &str) -> BTreeSet<&str> {
        let mut properties = BTreeSet::from([property.to_string()]);
        if self.canonical_options().logical_to_physical {
            properties.extend([
                property.replace("-top", "-block-start"),
                property.replace("-bottom", "-block-end"),
                property.replace("-left", "-inline-start"),
                property.replace("-right", "-inline-end"),
            ]);
        }
        let mut names = BTreeSet::new();
        for property in properties {
            let parts: Vec<_> = property.split('-').collect();
            if parts.len() > 6 {
                continue;
            }
            for mask in 1..1usize << parts.len() {
                let parent = parts
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, part)| *part)
                    .collect::<Vec<_>>()
                    .join("-");
                if !self.covers(&parent, &property) {
                    continue;
                }
                names.extend(
                    self.context
                        .design_system
                        .utility_search()
                        .names_for_property(&parent),
                );
            }
        }
        names
    }
}
