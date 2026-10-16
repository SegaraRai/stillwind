//! The resolved variants of one candidate, with their positions in its text.

use std::ops::Range;

use stillwind_syntax::Candidate;

use super::super::{VariantApplication, resolve_variant};
use crate::ThemeIndex;

/// One resolved variant of a candidate.
pub(super) struct StackVariant<'a> {
    /// Variant text as written, without its trailing `:`.
    pub(super) raw: &'a str,
    /// Byte range of [`Self::raw`] in the candidate text.
    pub(super) range: Range<usize>,
    /// The resolved application.
    pub(super) application: VariantApplication,
}

/// Resolves every variant of `candidate`, or returns `None` when one does not resolve.
pub(super) fn resolve_stack<'a>(
    candidate: &'a Candidate,
    theme: &ThemeIndex,
) -> Option<Vec<StackVariant<'a>>> {
    let mut offset = 0;
    candidate
        .variants
        .iter()
        .map(|variant| {
            let start = offset;
            offset += variant.raw.len() + 1;
            Some(StackVariant {
                raw: &variant.raw,
                range: start..start + variant.raw.len(),
                application: resolve_variant(variant, theme).ok()?,
            })
        })
        .collect()
}
