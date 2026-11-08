//! Optimizations of selector lists and `@keyframes` rules.

use lightningcss::rules::keyframes::{KeyframeSelector, KeyframesRule};
use lightningcss::selector::SelectorList;
use lightningcss::values::percentage::Percentage;

/// Removes each selector that `list` already contains earlier, as in `.a,.a`.
///
/// A rule matches an element when any selector in its list matches, and equal selectors
/// have equal specificity, so a repeated selector adds nothing.
pub(super) fn remove_duplicate_selectors(list: &mut SelectorList<'_>) {
    let mut index = 1;
    while index < list.0.len() {
        if list.0[..index].contains(&list.0[index]) {
            list.0.remove(index);
        } else {
            index += 1;
        }
    }
}

/// Moves the selectors of each keyframe to the first earlier keyframe that has equal
/// declarations, as in `0%{opacity:0}50%{opacity:0}` to `0%,50%{opacity:0}`.
///
/// When two keyframes select the same offset, the later one wins for the properties that
/// both set, so moving one of them could change the result. The rule stays unchanged
/// when any offset appears more than once, including `from` with `0%` and `to` with
/// `100%`.
pub(super) fn merge_keyframes(rule: &mut KeyframesRule<'_>) {
    let mut offsets: Vec<KeyframeSelector> = Vec::new();
    for selector in rule
        .keyframes
        .iter()
        .flat_map(|keyframe| &keyframe.selectors)
    {
        let offset = offset(selector);
        if offsets.contains(&offset) {
            return;
        }
        offsets.push(offset);
    }
    let keyframes = &mut rule.keyframes;
    let mut index = 0;
    while index < keyframes.len() {
        let mut other = index + 1;
        while other < keyframes.len() {
            if keyframes[other].declarations == keyframes[index].declarations {
                let merged = keyframes.remove(other);
                keyframes[index].selectors.extend(merged.selectors);
            } else {
                other += 1;
            }
        }
        index += 1;
    }
}

/// The selector that `selector` is an alias of, so that `from` equals `0%` and `to`
/// equals `100%`.
fn offset(selector: &KeyframeSelector) -> KeyframeSelector {
    match selector {
        KeyframeSelector::From => KeyframeSelector::Percentage(Percentage(0.0)),
        KeyframeSelector::To => KeyframeSelector::Percentage(Percentage(1.0)),
        other => other.clone(),
    }
}
