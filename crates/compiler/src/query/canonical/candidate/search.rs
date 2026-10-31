//! Searches for the canonical spelling of one candidate.
//!
//! Each step of the search tries four sources of replacements in order: the named
//! spelling of a dynamic family, local rewrites of the candidate, named aliases, and
//! the canonical index. The first replacement that produces the same CSS becomes the
//! input of the next step. The search stops when no source finds a new spelling.

use std::collections::BTreeSet;
use std::sync::PoisonError;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{UtilityResolutionError, dynamic_named_spelling};
use stillwind_diagnostics::CanonicalUtilitiesConfig;
use stillwind_syntax::{CandidatePrefix, Utility, parse_candidate};

use super::super::conditions::canonical_conditions;
use super::equivalence::ReplacementChoice;
use crate::compiler::candidate_allowed;
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Returns the options of the `prefer-canonical-utilities` rule that this query
    /// uses. The compiler defaults apply first, then the severity from the stylesheet,
    /// and then the options of the project.
    pub fn canonical_options(&self) -> CanonicalUtilitiesConfig {
        self.context.canonical_options()
    }

    /// Returns the canonical spelling of the candidate `raw`, such as `p-4` for
    /// `p-[16px]`. Returns `None` when `raw` is already canonical, or when no
    /// replacement produces the same CSS.
    ///
    /// `raw` must be one complete candidate whose arbitrary values are already decoded.
    /// The result uses the definitions of this query's snapshot and the options from
    /// `canonical_options`. A legacy spelling, or a candidate with an invalid type
    /// hint, may also receive a replacement.
    ///
    /// The query caches the results. Queries that `DesignSystemQuery::shared` creates
    /// for the same snapshot and options share one cache. Other queries have their own
    /// cache.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a utility definition that the search checks.
    pub fn canonical_candidate(
        &self,
        raw: &str,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
        if let Some(cached) = self
            .canonical_cache()
            .candidates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(raw)
        {
            return Ok(cached.clone());
        }
        let migrated = self.migrate_candidate(raw)?;
        let mut current = canonical_conditions(&migrated, self.context.design_system.theme())
            .map_or(migrated, Into::into);
        let mut visited = BTreeSet::new();
        while visited.insert(current.clone()) {
            let mut next = self.dynamic_named_candidate(&current)?;
            if next.is_none() {
                let mut choice = ReplacementChoice::default();
                for candidate in self.local_candidates(&current) {
                    if candidate == current {
                        continue;
                    }
                    if self.equivalent(&current, &candidate)? {
                        next = choice.offer(self, &current, &candidate)?;
                    } else if self.context.canonical_preview(&current)?.is_none()
                        && self.recover_hint(&current, &candidate)?
                        && self.context.canonical_preview(&candidate)?.is_some()
                    {
                        next = Some(candidate);
                    }
                    if next.is_some() {
                        break;
                    }
                }
                next = next.or(choice.finish());
            }
            if next.is_none() {
                next = self.named_alias(&current)?;
            }
            if next.is_none() {
                next = self.find_canonical(&current)?;
            }
            let Some(next) = next.filter(|next| !visited.contains(next)) else {
                break;
            };
            current = next;
        }
        let result = (current != raw).then_some(current).filter(|candidate| {
            candidate_allowed(
                candidate,
                self.context.design_system.theme(),
                &self.lint_settings(),
            )
        });
        self.canonical_cache()
            .candidates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(raw.to_lean_str(), result.clone());
        Ok(result)
    }

    /// Returns `raw` with its bracket form replaced by the named form, such as
    /// `icon-home` for `icon-[home]`. Returns `None` when there is no such replacement.
    ///
    /// The method replaces the name only when a dynamic family accepts both forms and
    /// resolves them to the same definition. When another definition accepts the
    /// bracket value, the bracket form stays. The complete replacement, with its
    /// variants and modifier, must resolve in the snapshot of this query.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a utility definition.
    fn dynamic_named_candidate(
        &self,
        raw: &str,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
        let Some(Utility::Named { name, .. }) = parse_candidate(raw).map(|parsed| parsed.utility)
        else {
            return Ok(None);
        };
        let Some(named) = dynamic_named_spelling(&name, self.context.design_system.theme())? else {
            return Ok(None);
        };
        let split = CandidatePrefix::variant_end(raw);
        let replacement = format_args!(
            "{}{}",
            &raw[..split],
            raw[split..].replacen(name.as_str(), &named, 1)
        )
        .to_lean_str();
        Ok(self
            .context
            .canonical_preview(&replacement)?
            .map(|_| replacement))
    }
}
