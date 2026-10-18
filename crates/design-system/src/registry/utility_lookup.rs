//! Selects exact, wildcard, and `(functional)` definitions while preserving each
//! registry layer's order.

use std::collections::BTreeMap;
use std::sync::Arc;

use lean_string::LeanStr;

use crate::CustomUtility;

/// Indexes into one registry's definition list, grouped by how a name matches them.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct UtilityLookup {
    /// Definitions without `*` and without the `(functional)` flag, by name.
    exact: BTreeMap<LeanStr, Vec<usize>>,
    /// Wildcard definitions by the prefix before their trailing `*`.
    wildcards: BTreeMap<LeanStr, Vec<usize>>,
    /// `(functional)` definitions by their complete candidate, in source order.
    functional: BTreeMap<LeanStr, Vec<usize>>,
}

impl UtilityLookup {
    /// Indexes `utilities`. The returned lookup answers with positions in that slice,
    /// so callers must pass the same slice to the query methods.
    pub(crate) fn new(utilities: &[Arc<CustomUtility>]) -> Self {
        let mut exact: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut wildcards: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut functional: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (index, utility) in utilities.iter().enumerate() {
            if utility.role.functional {
                functional
                    .entry(utility.name.clone())
                    .or_default()
                    .push(index);
            } else if let Some(prefix) = utility.name.strip_suffix('*') {
                wildcards
                    .entry(LeanStr::from(prefix))
                    .or_default()
                    .push(index);
            } else {
                exact.entry(utility.name.clone()).or_default().push(index);
            }
        }
        Self {
            exact,
            wildcards,
            functional,
        }
    }

    /// Whether a definition without the `(functional)` flag has exactly this name.
    pub(crate) fn has_exact(&self, name: &str) -> bool {
        self.exact.contains_key(name)
    }

    /// Returns the `(functional)` definitions of one complete candidate in source
    /// order.
    pub(crate) fn functional<'a>(
        &self,
        candidate: &str,
        utilities: &'a [Arc<CustomUtility>],
    ) -> Vec<&'a CustomUtility> {
        self.functional
            .get(candidate)
            .into_iter()
            .flatten()
            .map(|index| utilities[*index].as_ref())
            .collect()
    }

    /// Returns the definitions that the utility name `name` matches: the exact
    /// definitions of `name` and the wildcard definitions whose prefix `name` starts
    /// with. `(functional)` definitions are not included.
    ///
    /// The result lists the newest definition first. With `wildcards_first`, the
    /// wildcard definitions come first, newest first, and the exact definitions follow,
    /// newest first; otherwise both kinds are ordered together.
    pub(crate) fn matching<'a>(
        &self,
        name: &str,
        utilities: &'a [Arc<CustomUtility>],
        wildcards_first: bool,
    ) -> Vec<&'a CustomUtility> {
        let mut indices = Vec::new();
        for end in name
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(name.len()))
        {
            if let Some(group) = self.wildcards.get(&name[..end]) {
                indices.extend(group);
            }
        }
        let exact = self.exact.get(name).map_or(&[][..], Vec::as_slice);
        if !wildcards_first {
            indices.extend(exact);
        }
        indices.sort_unstable_by(|left: &usize, right| right.cmp(left));
        let mut result: Vec<_> = indices
            .into_iter()
            .map(|index| utilities[index].as_ref())
            .collect();
        if wildcards_first {
            result.extend(exact.iter().rev().map(|index| utilities[*index].as_ref()));
        }
        result
    }
}
