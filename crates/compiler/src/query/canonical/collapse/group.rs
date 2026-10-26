//! Groups the candidates of one source container and returns their canonical spellings.

use std::collections::BTreeSet;
use std::sync::PoisonError;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::UtilityResolutionError;
use stillwind_syntax::{CandidatePrefix, parse_candidate};

use crate::query::DesignSystemQuery;

/// Candidates of one source container that have the same variants and the same `!`.
pub(super) struct Group {
    /// Variants of every candidate, including the final `:`. It is empty when the
    /// candidates have no variants.
    pub(super) prefix: LeanStr,
    /// Whether every candidate ends with `!`.
    pub(super) important: bool,
    /// The candidates without `prefix` and without the final `!`.
    pub(super) values: Vec<LeanStr>,
    /// The output position of each candidate in `values`, in the same order. Each
    /// position is unique within the container. The output lists candidates by
    /// increasing position.
    pub(super) positions: Vec<usize>,
}

impl Group {
    /// Returns `base` with the variants and the `!` of this group.
    pub(super) fn decorate(&self, base: &str) -> LeanStr {
        format!(
            "{}{base}{}",
            self.prefix,
            if self.important { "!" } else { "" }
        )
        .into()
    }

    /// Returns every candidate of this group with its variants and `!`.
    pub(super) fn decorated(&self) -> Vec<LeanStr> {
        self.values
            .iter()
            .map(|value| self.decorate(value))
            .collect()
    }

    /// Returns a group with the same variants and `!`. It contains the candidates at
    /// the indices `indices`, in that order, with their positions.
    pub(super) fn subset(&self, indices: &[usize]) -> Self {
        Self {
            prefix: self.prefix.clone(),
            important: self.important,
            values: indices
                .iter()
                .map(|index| self.values[*index].clone())
                .collect(),
            positions: indices.iter().map(|index| self.positions[*index]).collect(),
        }
    }

    /// Returns a copy of this group in which `proposal` replaces the candidates at the
    /// indices `selected`. When `proposal` is already in the group, it keeps its
    /// position. Otherwise it is added with the position `next_position`.
    pub(super) fn replacement(
        &self,
        proposal: &LeanStr,
        selected: &[usize],
        next_position: usize,
    ) -> Self {
        let existing = self.values.iter().position(|value| value == proposal);
        let keep = |index: &usize| Some(*index) == existing || !selected.contains(index);
        let mut values: Vec<_> = self
            .values
            .iter()
            .enumerate()
            .filter(|(index, _)| keep(index))
            .map(|(_, value)| value.clone())
            .collect();
        let mut positions: Vec<_> = self
            .positions
            .iter()
            .enumerate()
            .filter(|(index, _)| keep(index))
            .map(|(_, position)| *position)
            .collect();
        if existing.is_none() {
            values.push(proposal.clone());
            positions.push(next_position);
        }
        Self {
            prefix: self.prefix.clone(),
            important: self.important,
            values,
            positions,
        }
    }
}

impl DesignSystemQuery {
    /// Returns the canonical spellings of `candidates`, the candidates of one source
    /// container such as one class string.
    ///
    /// The method first replaces each candidate with the result of
    /// `canonical_candidate`. A spelling that appears more than once is kept once. When
    /// the `collapse` option is enabled, the method then replaces two or more
    /// candidates that have the same variants and `!` with one utility that produces
    /// the same CSS, such as `w-4 h-4` with `size-4`.
    ///
    /// Unknown candidates stay in the result. Candidates that do not change keep their
    /// order. A new candidate is placed after the candidates that it does not replace.
    ///
    /// The query caches the results. Queries that `DesignSystemQuery::shared` creates
    /// for the same snapshot and options share one cache. Other queries have their own
    /// cache.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of an input candidate, or of a utility that the
    /// method tries as a replacement. A replacement that changes the CSS is skipped
    /// without an error.
    pub fn canonical_candidates(
        &self,
        candidates: &[LeanStr],
    ) -> Result<Vec<LeanStr>, UtilityResolutionError> {
        if let Some(cached) = self
            .canonical_cache()
            .sets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(candidates)
        {
            return Ok(cached.clone());
        }
        let output = self.uncached_canonical_candidates(candidates)?;
        self.canonical_cache()
            .sets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(candidates.to_vec(), output.clone());
        Ok(output)
    }

    /// Computes the result of `canonical_candidates` without reading or writing the
    /// cache.
    ///
    /// # Errors
    ///
    /// Returns the same errors as `canonical_candidates`.
    fn uncached_canonical_candidates(
        &self,
        candidates: &[LeanStr],
    ) -> Result<Vec<LeanStr>, UtilityResolutionError> {
        let mut groups: Vec<Group> = Vec::new();
        let mut unique = BTreeSet::new();
        let mut canonical = Vec::new();
        for raw in candidates {
            let raw = self
                .canonical_candidate(raw)?
                .unwrap_or_else(|| raw.clone());
            if unique.insert(raw.clone()) {
                canonical.push(raw);
            }
        }
        if !self.canonical_options().collapse || canonical.len() < 2 {
            return Ok(canonical);
        }
        let mut next_position = canonical.len();
        for (position, raw) in canonical.into_iter().enumerate() {
            let Some(parsed) = parse_candidate(&raw) else {
                groups.push(Group {
                    prefix: LeanStr::new(),
                    important: false,
                    values: vec![raw],
                    positions: vec![position],
                });
                continue;
            };
            let split = CandidatePrefix::variant_end(&raw);
            let prefix = raw[..split].to_lean_str();
            let base = raw[split..].trim_matches('!').to_lean_str();
            if let Some(group) = groups
                .iter_mut()
                .find(|group| group.prefix == prefix && group.important == parsed.important)
            {
                group.values.push(base);
                group.positions.push(position);
            } else {
                groups.push(Group {
                    prefix,
                    important: parsed.important,
                    values: vec![base],
                    positions: vec![position],
                });
            }
        }
        let mut output = Vec::new();
        for mut group in groups {
            self.collapse_group(&mut group, &mut next_position)?;
            output.extend(
                group
                    .values
                    .iter()
                    .zip(&group.positions)
                    .map(|(base, position)| (*position, group.decorate(base))),
            );
        }
        output.sort_by_key(|(position, _)| *position);
        Ok(output.into_iter().map(|(_, value)| value).collect())
    }
}
