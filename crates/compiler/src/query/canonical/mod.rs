//! Finds equivalent canonical spellings under the active lint policy.
//!
//! `candidate` implements canonical queries and verifies proposed replacements.
//! `preview` renders and normalizes CSS for comparison. `index` prepares finite
//! spellings, `values` indexes theme values, and `variants` indexes named wrappers.
//! Call [`crate::DesignSystemQuery::canonical_candidate`] to query a candidate.

mod candidate;
mod index;
mod preview;
mod values;
mod variants;

pub(super) use index::CanonicalIndex;
pub(super) use preview::CanonicalPreview;
