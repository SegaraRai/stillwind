//! Descriptions and explanations for `conflicting-utilities` findings.
//!
//! Callers in [`crate::style_analysis`] and the stylesheet lint detect competing
//! declarations of two utility candidates and pass the property pairs here.
//! [`describe_overlaps`] formats the property list of the message, and
//! [`conflict_notes`] explains the overlap from W3C Webref property data and the rule
//! settings.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{property_covers, writing_mode_group};
use stillwind_source::JoinText;

/// Competing declarations of two candidates, as `(previous property, current property)`.
pub(crate) type PropertyPairs = BTreeSet<(LeanStr, LeanStr)>;

/// Returns the property list of the message, with `a` for one property that both
/// candidates set and `b and a` for interfering properties, in set order.
pub(crate) fn describe_overlaps(pairs: &PropertyPairs) -> LeanStr {
    pairs
        .iter()
        .map(|(previous, current)| {
            if previous == current {
                previous.clone()
            } else {
                format_args!("{previous} and {current}").to_lean_str()
            }
        })
        .collect::<Vec<_>>()
        .join_text(", ")
        .into()
}

/// Returns notes that explain the overlap of `pairs`, which must be non-empty.
/// `allow_shorthand_overrides` is the rule option that was in effect.
///
/// When every pair is a shorthand and a property that it covers, the note names the
/// option that allows the pair. Otherwise, each pair that overlaps only through a
/// Webref logical group receives a note that names the group.
pub(crate) fn conflict_notes(
    pairs: &PropertyPairs,
    allow_shorthand_overrides: bool,
) -> Vec<LeanStr> {
    if !allow_shorthand_overrides
        && pairs.iter().all(|(previous, current)| {
            property_covers(previous, current) || property_covers(current, previous)
        })
    {
        return vec![
            "set `allowShorthandOverrides` of `conflicting-utilities` to let a longhand utility \
             override part of a shorthand utility"
                .into(),
        ];
    }
    let mut notes = Vec::new();
    for (previous, current) in pairs {
        if property_covers(previous, current) || property_covers(current, previous) {
            continue;
        }
        if let Some(group) = writing_mode_group(previous, current) {
            let note = writing_mode_note(previous, current, group);
            if !notes.contains(&note) {
                notes.push(note);
            }
        }
    }
    notes
}

/// Returns the note for `previous` and `current`, properties that overlap only through
/// the Webref logical group `group`. [`crate::DesignSystemQuery::explain_conflicts`]
/// finds this note again to append a replacement.
pub(crate) fn writing_mode_note(previous: &str, current: &str, group: &str) -> LeanStr {
    format_args!(
        "`{previous}` and `{current}` overlap through the `{group}` logical group, so the writing mode decides which sides they share; set both with logical properties or both with physical properties"
    )
    .to_lean_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(items: &[(&str, &str)]) -> PropertyPairs {
        items
            .iter()
            .map(|(previous, current)| ((*previous).into(), (*current).into()))
            .collect()
    }

    #[test]
    fn names_the_shorthand_option_only_when_it_would_allow_every_pair() {
        assert_eq!(
            conflict_notes(&pairs(&[("margin", "margin-block-end")]), false),
            [
                "set `allowShorthandOverrides` of `conflicting-utilities` to let a longhand \
                 utility override part of a shorthand utility"
            ]
        );
        assert_eq!(
            conflict_notes(&pairs(&[("margin", "margin-block-end")]), true),
            [] as [&str; 0]
        );
        assert_eq!(
            conflict_notes(&pairs(&[("padding", "padding")]), false),
            [] as [&str; 0]
        );
    }

    #[test]
    fn names_the_logical_group_of_a_writing_mode_overlap() {
        assert_eq!(
            conflict_notes(&pairs(&[("padding-inline", "padding-top")]), false),
            [
                "`padding-inline` and `padding-top` overlap through the `padding` logical \
                 group, so the writing mode decides which sides they share; set both with \
                 logical properties or both with physical properties"
            ]
        );
        assert_eq!(
            conflict_notes(&pairs(&[("line-height", "line-height")]), false),
            [] as [&str; 0]
        );
    }
}
