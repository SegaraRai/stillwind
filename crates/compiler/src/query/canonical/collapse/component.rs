//! Accepts proposals for one group, one proposal at a time.

use std::collections::BTreeMap;

use lean_string::LeanStr;

use stillwind_design_system::UtilityResolutionError;

use super::compare::CanonicalSet;
use super::group::Group;
use super::partition::{Acceptance, Component, Footprint};
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Replaces candidates in `group` with proposals until no proposal can be accepted.
    ///
    /// Each round tries the proposals in `rank` order and accepts the first proposal
    /// that keeps the CSS of `group`. When an accepted proposal is a new candidate, it
    /// gets the position `next_position`, and `next_position` increases by one.
    ///
    /// `outside` lists the other candidates of the same group, with their indices in
    /// the complete group. The caller must ensure that no candidate in `outside` can
    /// change the CSS of a candidate in `group`.
    ///
    /// Returns `Accepted` with the accepted proposals in the order of acceptance.
    /// Returns `Related` with an index when a proposal that fits `group` can change the
    /// CSS of that candidate in `outside`. Returns `Incomparable` when the CSS of
    /// `group` cannot be compared and `outside` is not empty. After `Related` or
    /// `Incomparable`, `group` may be partly changed, so the caller must discard it.
    /// When `outside` is empty, the result is always `Accepted`.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a candidate or of a proposal.
    pub(super) fn collapse_component(
        &self,
        group: &mut Group,
        next_position: &mut usize,
        outside: &[(usize, &Footprint)],
    ) -> Result<Component, UtilityResolutionError> {
        let mut acceptances = Vec::new();
        let mut related = BTreeMap::new();
        loop {
            if group.values.len() < 2 {
                return Ok(Component::Accepted(acceptances));
            }
            let declarations: Vec<_> = group
                .values
                .iter()
                .map(|value| self.candidate_declarations(value))
                .collect();
            let proposals = self.collapse_proposals(&group.values, &declarations);
            if proposals.is_empty() {
                return Ok(Component::Accepted(acceptances));
            }
            let before: Vec<_> = group.decorated();
            let Some(reference) = self.canonical_set(&before)? else {
                return Ok(if outside.is_empty() || !acceptances.is_empty() {
                    Component::Accepted(acceptances)
                } else {
                    Component::Incomparable
                });
            };
            let mut replacement = None;
            for proposal in proposals {
                let Some(properties) = self.candidate_declarations(&proposal) else {
                    continue;
                };
                if !self.proposal_fits(&properties, &declarations) {
                    continue;
                }
                if let Some(member) = *related
                    .entry(proposal.clone())
                    .or_insert_with(|| self.related_outside(&proposal, outside))
                {
                    return Ok(Component::Related(member));
                }
                let selected: Vec<_> = declarations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, source)| {
                        let source = source.as_ref()?;
                        (!source.is_empty()
                            && source.iter().all(|declaration| {
                                properties.iter().any(|target| {
                                    self.covers(&target.property, &declaration.property)
                                })
                            }))
                        .then_some(index)
                    })
                    .collect();
                if selected.len() < 2 {
                    continue;
                }
                let Some(target) = self.verified_replacement(
                    group,
                    &proposal,
                    &selected,
                    &reference,
                    *next_position,
                )?
                else {
                    continue;
                };
                let created = target.positions.contains(next_position);
                acceptances.push(Acceptance {
                    proposal: proposal.clone(),
                    position: created.then_some(*next_position),
                });
                if created {
                    *next_position += 1;
                }
                replacement = Some(target);
                break;
            }
            let Some(replacement) = replacement else {
                return Ok(Component::Accepted(acceptances));
            };
            *group = replacement;
        }
    }

    /// Returns a copy of `group` in which `proposal` replaces the candidates at the
    /// indices `selected`. Returns `None` when this replacement changes the CSS of
    /// `group`.
    ///
    /// When `proposal` has a different canonical spelling, the copy uses that spelling
    /// if the CSS still stays the same. `reference` must be the result of
    /// `canonical_set` for `group`. When `proposal` is a new candidate, it gets the
    /// position `next_position`.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a candidate or of `proposal`.
    fn verified_replacement(
        &self,
        group: &Group,
        proposal: &LeanStr,
        selected: &[usize],
        reference: &CanonicalSet,
        next_position: usize,
    ) -> Result<Option<Group>, UtilityResolutionError> {
        // Comparing the whole group for every proposal takes seconds when one call
        // contains hundreds of utilities. This comparison skips the proposal unless
        // the replaced utilities alone produce its CSS. The whole group comparison
        // below still decides the replacement, because other utilities in the group
        // may override the same properties.
        let replaced: Vec<_> = selected
            .iter()
            .map(|index| group.decorate(&group.values[*index]))
            .collect();
        let Some(replaced) = self.canonical_set(&replaced)? else {
            return Ok(None);
        };
        if !self
            .canonical_set(&[group.decorate(proposal)])?
            .is_some_and(|proposal| replaced.matches(&proposal))
        {
            return Ok(None);
        }
        let keeps_css = |target: &Group| -> Result<bool, UtilityResolutionError> {
            Ok(self
                .canonical_set(&target.decorated())?
                .is_some_and(|after| reference.matches(&after)))
        };
        let target = group.replacement(proposal, selected, next_position);
        if !keeps_css(&target)? {
            return Ok(None);
        }
        let canonical = self
            .canonical_candidate(proposal)?
            .unwrap_or(proposal.clone());
        if canonical == *proposal {
            return Ok(Some(target));
        }
        let target = group.replacement(&canonical, selected, next_position);
        Ok(keeps_css(&target)?.then_some(target))
    }
}
