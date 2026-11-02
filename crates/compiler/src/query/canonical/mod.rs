//! Finds the canonical spelling of candidates under the options of the
//! `prefer-canonical-utilities` rule.
//!
//! A canonical spelling produces the same CSS as the original candidate, and the rule
//! prefers it, such as `p-4` for `p-[16px]`. Call
//! [`crate::DesignSystemQuery::canonical_candidate`] for one candidate, or
//! [`crate::DesignSystemQuery::canonical_candidates`] for all candidates of one source
//! container.
//!
//! Modules:
//!
//! - [`candidate`] finds the canonical spelling of one candidate.
//! - [`collapse`] replaces several candidates of one source container with one utility.
//! - [`conditions`] rewrites runtime conditions.
//! - [`index`] lists the named spellings that a search can return.
//! - [`parallel`] runs independent preparation steps on several threads.
//! - [`preview`] renders the CSS of a candidate and normalizes it for comparison.
//! - [`selectors`] writes equivalent selectors in one spelling for comparison.
//! - [`values`] lists theme values.
//! - [`variants`] lists named variants.

mod candidate;
mod collapse;
mod conditions;
mod index;
mod parallel;
mod preview;
mod selectors;
mod values;
mod variants;

pub(super) use index::CanonicalIndex;
pub(super) use preview::CanonicalPreview;

#[cfg(test)]
mod tests;
