//! Volar interval loading with exact-text validation and boundary ownership.
//!
//! Equal-text intervals support interior lookup; nonlinear intervals supply only
//! boundary anchors. A right-hand interval owns its start ahead of a previous end.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::index::{LinearRange, ProjectionError, ProjectionIndex};
use crate::{CoordinateIndex, SourceFile, Span, Utf16Offset};

/// One batch of parallel Volar arrays.
///
/// Present arrays must have equal lengths and describe corresponding entries by index.
/// Entries need not be globally sorted; construction does not validate coordinates.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolarMapping {
    /// UTF-16 start offsets in the original text.
    pub source_offsets: Vec<u32>,
    /// UTF-16 start offsets in the generated text.
    pub generated_offsets: Vec<u32>,
    /// Original-side lengths in UTF-16 units; `0` marks a boundary anchor.
    pub lengths: Vec<u32>,
    /// Generated-side UTF-16 lengths; None reuses lengths. May differ from original
    /// lengths to represent rewritten text; zero denotes a boundary anchor.
    #[serde(default)]
    pub generated_lengths: Option<Vec<u32>>,
}

/// Loads parallel UTF-16 arrays, rejecting malformed lengths, overflow, and coordinates.
#[allow(clippy::too_many_lines)] // Interval validation and boundary precedence form one pass.
pub(super) fn load<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    mappings: &[VolarMapping],
) -> Result<ProjectionIndex<'a>, ProjectionError> {
    let generated_index = CoordinateIndex::new(generated);
    let original_index = CoordinateIndex::new(original);
    let mut result = ProjectionIndex {
        generated,
        original,
        points: BTreeMap::new(),
        ranges: Vec::new(),
    };
    let mut exclusive_ends = Vec::new();
    for mapping in mappings {
        let count = mapping.lengths.len();
        if mapping.source_offsets.len() != count
            || mapping.generated_offsets.len() != count
            || mapping
                .generated_lengths
                .as_ref()
                .is_some_and(|lengths| lengths.len() != count)
        {
            return Err(ProjectionError::Map("Volar array lengths differ".into()));
        }
        for index in 0..count {
            let source_length = mapping.lengths[index];
            let generated_length = mapping
                .generated_lengths
                .as_ref()
                .map_or(source_length, |lengths| lengths[index]);
            let end = |start: u32, length: u32| {
                start
                    .checked_add(length)
                    .ok_or_else(|| ProjectionError::Map("Volar offset overflow".into()))
            };
            let original_start = original_index
                .byte(Utf16Offset(mapping.source_offsets[index]))?
                .0;
            let original_end = original_index
                .byte(Utf16Offset(end(
                    mapping.source_offsets[index],
                    source_length,
                )?))?
                .0;
            let generated_start = generated_index
                .byte(Utf16Offset(mapping.generated_offsets[index]))?
                .0;
            let generated_end = generated_index
                .byte(Utf16Offset(end(
                    mapping.generated_offsets[index],
                    generated_length,
                )?))?
                .0;
            // Empty anchors describe a boundary, not the character
            // beginning there. A nonempty interval owns that character.
            if source_length == 0 && generated_length == 0 {
                exclusive_ends.push((generated_start, original_start));
                continue;
            }
            // Nonlinear intervals supply boundary anchors, never an invented scale.
            if source_length == generated_length {
                let generated_span = Span::new(generated.id(), generated_start, generated_end);
                let original_span = Span::new(original.id(), original_start, original_end);
                if generated.slice(generated_span)? == original.slice(original_span)? {
                    result.ranges.push(LinearRange {
                        generated: generated_span,
                        original: original_span,
                    });
                }
            }
            let start = if generated_length == 0 && source_length != 0 {
                None
            } else {
                Some(original_start)
            };
            result
                .points
                .entry(generated_start)
                .and_modify(|previous| {
                    if *previous != start {
                        *previous = None;
                    }
                })
                .or_insert(start);
            exclusive_ends.push((generated_end, original_end));
        }
    }
    result.ranges.sort_by_key(|range| range.generated.start.0);
    // A right-hand interval owns its first character. The previous
    // interval's exclusive end is only a fallback at an uncovered boundary.
    let starts = result.points.keys().copied().collect::<BTreeSet<_>>();
    for (generated_end, original_end) in exclusive_ends {
        if starts.contains(&generated_end)
            || result.ranges.iter().any(|range| {
                range.generated.start.0 <= generated_end && generated_end < range.generated.end.0
            })
        {
            continue;
        }
        result
            .points
            .entry(generated_end)
            .and_modify(|previous| {
                if *previous != Some(original_end) {
                    *previous = None;
                }
            })
            .or_insert(Some(original_end));
    }
    Ok(result)
}
