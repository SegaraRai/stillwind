//! Selects exact and wildcard definitions while preserving each registry layer's order.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::CustomUtility;

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct UtilityLookup {
    exact: BTreeMap<String, Vec<usize>>,
    wildcards: BTreeMap<String, Vec<usize>>,
}

impl UtilityLookup {
    pub(crate) fn new(utilities: &[Arc<CustomUtility>]) -> Self {
        let mut exact: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut wildcards: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (index, utility) in utilities.iter().enumerate() {
            if let Some(prefix) = utility.name.strip_suffix('*') {
                wildcards.entry(prefix.to_owned()).or_default().push(index);
            } else {
                exact.entry(utility.name.clone()).or_default().push(index);
            }
        }
        Self { exact, wildcards }
    }

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
