//! Writing-mode resolution of Webref logical groups.
//!
//! [`properties_interfere`] treats a logical property as competing with every physical
//! property of its Webref logical group, because some writing mode maps it there.
//! [`properties_interfere_in`] narrows this to the writing modes that a caller selects
//! with [`WritingMode`]. It maps each logical longhand to the physical longhand that it
//! sets in those modes, in both the `ltr` and the `rtl` direction.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};
use serde::{Deserialize, Serialize};

use super::webref::{
    crosses_logical_boundary, is_logical, logical_group, longhand_family, properties_interfere,
    webref_property,
};

/// A value of the CSS `writing-mode` property that decides how logical sides map to
/// physical sides.
///
/// The `ltr` and `rtl` directions are always considered together. With both directions,
/// `sideways-rl` maps sides as `vertical-rl` does, and `sideways-lr` maps sides as
/// `vertical-lr` does, so these three values cover every writing mode. Each variant
/// serializes as its CSS keyword, such as `horizontal-tb`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WritingMode {
    /// `horizontal-tb`: the block axis runs from top to bottom.
    HorizontalTb,
    /// `vertical-rl` and `sideways-rl`: the block axis runs from right to left.
    VerticalRl,
    /// `vertical-lr` and `sideways-lr`: the block axis runs from left to right.
    VerticalLr,
}

impl WritingMode {
    /// Every writing mode, in declaration order.
    pub const ALL: [Self; 3] = [Self::HorizontalTb, Self::VerticalRl, Self::VerticalLr];

    /// Returns the CSS keyword of this writing mode, such as `horizontal-tb`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HorizontalTb => "horizontal-tb",
            Self::VerticalRl => "vertical-rl",
            Self::VerticalLr => "vertical-lr",
        }
    }

    /// Returns the writing mode whose CSS keyword is `name`, compared exactly, or `None`
    /// for any other text.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }

    /// Returns the physical sides and axes of this writing mode in the `ltr` direction
    /// when `rtl` is false, or in the `rtl` direction when it is true.
    const fn sides(self, rtl: bool) -> Sides {
        let (block_start, block_end, block_axis, block_size) = match self {
            Self::HorizontalTb => ("top", "bottom", "y", "height"),
            Self::VerticalRl => ("right", "left", "x", "width"),
            Self::VerticalLr => ("left", "right", "x", "width"),
        };
        let (inline_start, inline_end, inline_axis, inline_size) = match (self, rtl) {
            (Self::HorizontalTb, false) => ("left", "right", "x", "width"),
            (Self::HorizontalTb, true) => ("right", "left", "x", "width"),
            (_, false) => ("top", "bottom", "y", "height"),
            (_, true) => ("bottom", "top", "y", "height"),
        };
        Sides {
            block: Axis {
                start: block_start,
                end: block_end,
                name: block_axis,
                size: block_size,
            },
            inline: Axis {
                start: inline_start,
                end: inline_end,
                name: inline_axis,
                size: inline_size,
            },
        }
    }
}

/// The physical names of the block and inline axes in one writing mode and direction.
struct Sides {
    block: Axis,
    inline: Axis,
}

/// The physical names that one logical axis maps to.
struct Axis {
    /// The side at the start of the axis, such as `top`.
    start: &'static str,
    /// The side at the end of the axis, such as `bottom`.
    end: &'static str,
    /// The axis suffix of physical properties, `x` or `y`.
    name: &'static str,
    /// The physical dimension along the axis, `width` or `height`.
    size: &'static str,
}

impl Axis {
    /// Returns the physical side for the logical segment `start` or `end`.
    fn side(&self, segment: &str) -> Option<&'static str> {
        match segment {
            "start" => Some(self.start),
            "end" => Some(self.end),
            _ => None,
        }
    }
}

impl Sides {
    /// Returns the axis for the logical segment `block` or `inline`.
    fn axis(&self, segment: &str) -> Option<&Axis> {
        match segment {
            "block" => Some(&self.block),
            "inline" => Some(&self.inline),
            _ => None,
        }
    }
}

/// Whether two CSS properties compete for one declaration slot in at least one of
/// `modes`.
///
/// The result equals [`properties_interfere`], except for a logical property and a
/// physical property that share no longhand and meet only through a Webref logical
/// group. Such a pair interferes when the logical property sets a physical longhand of
/// the other property in one of `modes`, in the `ltr` or the `rtl` direction. A pair
/// that overlaps in every writing mode, such as `padding` and `padding-inline`,
/// interferes even when `modes` is empty.
///
/// For example, `padding-inline` and `padding-top` interfere in `vertical-rl` but not in
/// `horizontal-tb`, and `padding-inline-start` and `padding-left` interfere in
/// `horizontal-tb` because the `ltr` direction maps the start to the left.
#[must_use]
pub fn properties_interfere_in(left: &str, right: &str, modes: &[WritingMode]) -> bool {
    if !properties_interfere(left, right) {
        return false;
    }
    let left = longhand_family(left);
    let right = longhand_family(right);
    if left.iter().any(|name| right.contains(name)) || !crosses_logical_boundary(&left, &right) {
        return true;
    }
    WritingMode::ALL
        .iter()
        .all(|&mode| overlaps_in(&left, &right, mode))
        || modes.iter().any(|&mode| overlaps_in(&left, &right, mode))
}

/// Whether the longhand families `left` and `right` set one physical longhand in `mode`
/// with the `ltr` or the `rtl` direction.
fn overlaps_in(left: &BTreeSet<LeanStr>, right: &BTreeSet<LeanStr>, mode: WritingMode) -> bool {
    [false, true].into_iter().any(|rtl| {
        let sides = mode.sides(rtl);
        let right: BTreeSet<_> = right.iter().map(|name| physical(name, &sides)).collect();
        left.iter()
            .any(|name| right.contains(&physical(name, &sides)))
    })
}

/// Returns the physical longhand that `name` sets with `sides`, or `name` itself when it
/// is not a logical member of a Webref logical group.
fn physical(name: &str, sides: &Sides) -> LeanStr {
    physical_longhand(name, sides).unwrap_or(name).to_lean_str()
}

/// Returns the physical member of the Webref logical group of `name` that `name` sets
/// with `sides`, or `None` when `name` is not a logical member of a group.
///
/// The name segments map as follows: `block-start` and similar pairs become a side,
/// `start-end` and similar corner pairs become a corner such as `top-right`,
/// `block-size` and `inline-size` become `width` or `height`, and a final `block` or
/// `inline` becomes `x` or `y`.
fn physical_longhand(name: &str, sides: &Sides) -> Option<&'static str> {
    let group = logical_group(name)?;
    if !is_logical(name) {
        return None;
    }
    let segments: Vec<&str> = name.split('-').collect();
    let mut physical: Vec<&str> = Vec::with_capacity(segments.len() + 1);
    let mut index = 0;
    while let Some(&segment) = segments.get(index) {
        let next = segments.get(index + 1).copied();
        if let (Some(block), Some(inline)) = (
            sides.block.side(segment),
            next.and_then(|next| sides.inline.side(next)),
        ) {
            // Physical corner names put the vertical side first, as in `top-right`.
            if matches!(block, "top" | "bottom") {
                physical.extend([block, inline]);
            } else {
                physical.extend([inline, block]);
            }
            index += 2;
        } else if let Some(axis) = sides.axis(segment) {
            if let Some(side) = next.and_then(|next| axis.side(next)) {
                physical.push(side);
                index += 2;
            } else if next == Some("size") {
                physical.push(axis.size);
                index += 2;
            } else {
                physical.push(axis.name);
                index += 1;
            }
        } else {
            physical.push(segment);
            index += 1;
        }
    }
    let candidate = physical.join("-");
    // The physical members of `inset` drop the group name, as in `top`.
    [
        Some(candidate.as_str()),
        candidate.strip_prefix(&format!("{group}-")),
    ]
    .into_iter()
    .flatten()
    .filter_map(webref_property)
    .find(|definition| definition.logical_group == Some(group) && !is_logical(definition.name))
    .map(|definition| definition.name)
}

#[cfg(test)]
mod tests {
    use super::super::webref::WEBREF_PROPERTIES;
    use super::*;

    const HORIZONTAL: &[WritingMode] = &[WritingMode::HorizontalTb];
    const VERTICAL_RL: &[WritingMode] = &[WritingMode::VerticalRl];
    const VERTICAL_LR: &[WritingMode] = &[WritingMode::VerticalLr];

    #[test]
    fn every_logical_longhand_maps_to_a_physical_member_of_its_group() {
        for definition in WEBREF_PROPERTIES {
            if definition.logical_group.is_none()
                || !definition.longhands.is_empty()
                || !is_logical(definition.name)
            {
                continue;
            }
            for mode in WritingMode::ALL {
                for rtl in [false, true] {
                    assert!(
                        physical_longhand(definition.name, &mode.sides(rtl)).is_some(),
                        "{} in {} with rtl={rtl}",
                        definition.name,
                        mode.name()
                    );
                }
            }
        }
    }

    #[test]
    fn maps_sides_corners_sizes_and_axes() {
        let horizontal_rtl = WritingMode::HorizontalTb.sides(true);
        let vertical_rl = WritingMode::VerticalRl.sides(false);
        assert_eq!(
            physical_longhand("padding-inline-start", &horizontal_rtl),
            Some("padding-right")
        );
        assert_eq!(
            physical_longhand("inset-block-start", &vertical_rl),
            Some("right")
        );
        assert_eq!(
            physical_longhand("border-start-end-radius", &horizontal_rtl),
            Some("border-top-left-radius")
        );
        assert_eq!(
            physical_longhand("corner-start-start-shape", &vertical_rl),
            Some("corner-top-right-shape")
        );
        assert_eq!(
            physical_longhand("max-block-size", &vertical_rl),
            Some("max-width")
        );
        assert_eq!(
            physical_longhand("overflow-inline", &vertical_rl),
            Some("overflow-y")
        );
        assert_eq!(physical_longhand("padding-top", &vertical_rl), None);
    }

    #[test]
    fn logical_and_physical_properties_interfere_only_in_selected_modes() {
        assert!(!properties_interfere_in(
            "padding-inline",
            "padding-top",
            HORIZONTAL
        ));
        assert!(properties_interfere_in(
            "padding-inline",
            "padding-top",
            VERTICAL_RL
        ));
        assert!(properties_interfere_in(
            "margin-inline",
            "margin-left",
            HORIZONTAL
        ));
        assert!(!properties_interfere_in(
            "margin-inline",
            "margin-left",
            VERTICAL_RL
        ));
        assert!(properties_interfere_in(
            "padding-inline-start",
            "padding-left",
            HORIZONTAL
        ));
        assert!(properties_interfere_in(
            "padding-inline-start",
            "padding-right",
            HORIZONTAL
        ));
        assert!(!properties_interfere_in(
            "padding-block-start",
            "padding-bottom",
            &WritingMode::ALL
        ));
        assert!(properties_interfere_in(
            "padding-block-start",
            "padding-left",
            VERTICAL_LR
        ));
        assert!(!properties_interfere_in(
            "padding-block-start",
            "padding-left",
            VERTICAL_RL
        ));
        assert!(properties_interfere_in("width", "inline-size", HORIZONTAL));
        assert!(!properties_interfere_in("width", "block-size", HORIZONTAL));
        assert!(properties_interfere_in("width", "block-size", VERTICAL_RL));
    }

    #[test]
    fn overlaps_in_every_mode_and_shared_longhands_ignore_the_selection() {
        assert!(properties_interfere_in("padding", "padding-inline", &[]));
        assert!(properties_interfere_in("margin", "margin-block-start", &[]));
        assert!(properties_interfere_in("padding", "padding-top", &[]));
        assert!(properties_interfere_in("all", "color", &[]));
        assert!(!properties_interfere_in(
            "padding-inline",
            "padding-top",
            &[]
        ));
        assert!(!properties_interfere_in(
            "margin",
            "padding",
            &WritingMode::ALL
        ));
    }

    #[test]
    fn serializes_as_css_keywords() {
        for mode in WritingMode::ALL {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, format!("\"{}\"", mode.name()));
        }
    }
}
