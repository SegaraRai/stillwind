//! Finds where each part of a candidate is in the text of the candidate. Error messages
//! use these ranges to point at the part that failed.

use std::ops::Range;

use stillwind_syntax::{Candidate, Utility, parse_candidate};

/// The byte offsets of the parts of a parsed candidate in its text.
pub(super) struct CandidateLayout {
    /// The start of each variant, in the order of the variants.
    pub(super) variants: Vec<usize>,
    /// The leading `-` of a negative utility. It is `None` for other utilities.
    pub(super) negative: Option<Range<usize>>,
    /// The utility name or the arbitrary property, without `!`, `-`, and modifier.
    pub(super) name: Range<usize>,
    /// The modifier, including its leading `/`. It is `None` when the candidate has no
    /// modifier.
    pub(super) modifier: Option<Range<usize>>,
}

impl CandidateLayout {
    /// Finds the parts of `candidate` in `raw`. `raw` must be the text that `candidate`
    /// was parsed from.
    pub(super) fn new(raw: &str, candidate: &Candidate) -> Self {
        let mut offset = 0;
        let variants = candidate
            .variants
            .iter()
            .map(|variant| {
                let start = offset;
                offset += variant.raw.len() + 1;
                start
            })
            .collect();
        let mut end = raw.len();
        if raw[offset..].starts_with('!') {
            offset += 1;
        } else if candidate.important {
            end -= 1;
        }
        let negative = candidate.negative.then(|| {
            offset += 1;
            offset - 1..offset
        });
        let name_end = match &candidate.utility {
            Utility::Named {
                name,
                modifier: Some(_),
            } => offset + name.len(),
            _ => end,
        };
        Self {
            variants,
            negative,
            name: offset..name_end,
            modifier: (name_end < end).then_some(name_end..end),
        }
    }
}

/// Returns the byte range of the arbitrary value of `candidate` in `raw`. `raw` must be
/// the text that `candidate` was parsed from.
///
/// For a named utility, the range covers the last value in brackets or parentheses,
/// such as `[1/2]` in `md:w-[1/2]` or `(--x)` in `bg-(--x)`. For an arbitrary property,
/// it covers the text after the colon, such as `1/2` in `[width:1/2]`. When there is no
/// such value, the range covers the whole utility.
pub(super) fn arbitrary_value_range(raw: &str, candidate: &Candidate) -> Range<usize> {
    let name = CandidateLayout::new(raw, candidate).name;
    let text = &raw[name.clone()];
    let start = match candidate.utility {
        Utility::ArbitraryProperty { .. } => text.find(':').map(|colon| (colon + 1, 1)),
        Utility::Named { .. } if text.ends_with(')') => text.rfind("-(").map(|dash| (dash + 1, 0)),
        Utility::Named { .. } => text.rfind("-[").map(|dash| (dash + 1, 0)),
    };
    start.map_or(name.clone(), |(start, trailing)| {
        name.start + start..name.end - trailing
    })
}

/// Returns the byte range of the value of the utility `raw`.
///
/// For a named arbitrary value, the range includes its brackets or parentheses. For an
/// arbitrary property, it covers the text after the colon, without the closing bracket.
/// For other named utilities, it covers the text after the last `-`, such as `150` in
/// `opacity-150`. Returns `None` when `raw` is not a candidate, or when a named utility
/// without an arbitrary value has no `-`.
#[must_use]
pub fn utility_value_range(raw: &str) -> Option<Range<usize>> {
    let candidate = parse_candidate(raw)?;
    let name = CandidateLayout::new(raw, &candidate).name;
    let text = &raw[name.clone()];
    match candidate.utility {
        Utility::Named { .. } if !text.ends_with([']', ')']) => {
            let dash = text.rfind('-')?;
            Some(name.start + dash + 1..name.end)
        }
        _ => Some(arbitrary_value_range(raw, &candidate)),
    }
}

/// Returns the byte range of the modifier of `candidate` in `raw`, without its leading
/// `/`. Returns `None` when there is no modifier. `raw` must be the text that
/// `candidate` was parsed from.
pub(super) fn modifier_value_range(raw: &str, candidate: &Candidate) -> Option<Range<usize>> {
    let modifier = CandidateLayout::new(raw, candidate).modifier?;
    Some(modifier.start + 1..modifier.end)
}
