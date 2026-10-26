//! Finds the canonical spelling of candidates under the options of the
//! `prefer-canonical-utilities` rule. A canonical spelling produces the same CSS as the
//! original candidate, and it is the spelling that the rule prefers, such as `p-4` for
//! `p-[16px]`.
//!
//! [`candidate`] finds the canonical spelling of one candidate. [`collapse`] replaces
//! several candidates of one source container with one utility. [`conditions`] rewrites
//! runtime conditions. [`preview`] renders the CSS of a candidate and normalizes it for
//! comparison. [`index`] lists the named spellings that a search can return. [`values`]
//! lists theme values, and `variants` lists named variants. [`parallel`] runs
//! independent preparation steps on several threads. [`selectors`] writes equivalent
//! selectors in one spelling for comparison.
//!
//! Call [`crate::DesignSystemQuery::canonical_candidate`] for one candidate. Call
//! [`crate::DesignSystemQuery::canonical_candidates`] for all candidates of one source
//! container.

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
