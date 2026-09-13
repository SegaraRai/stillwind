//! Validated correspondence from generated JavaScript back to the original source.
//!
//! A [`ProjectionMap`] is one of the two transport formats a host converter produces: a
//! source map v3 document (points) or a Volar-style mapping table (intervals).
//! [`ProjectionIndex`] loads either format against the generated (shadow) and original
//! (editable) [`SourceFile`]s and answers queries about the correspondence between
//! them.
//!
//! Exact-span verification succeeds only when the mapped original text equals the
//! retained projected text. Coordinate lookup alone does not verify text equality.
//! Converters rewrite code, so a correspondence that merely looks plausible must fail
//! rather than produce an edit against the wrong characters.

use std::collections::BTreeMap;

use oxc_sourcemap::SourceMap;
use serde::{Deserialize, Serialize};

use crate::{
    BytePos, CoordinateError, CoordinateIndex, Position, SourceError, SourceFile, Span, Utf16Offset,
};

/// A projection map in the shape the host converter produced it.
///
/// The variant decides how offsets are read: v3 tokens are points in UTF-16
/// coordinates, Volar entries are intervals in UTF-16 coordinates. Neither variant is
/// normalized into the other.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectionMap {
    /// A source map v3 document plus the source name that identifies the
    /// original file.
    V3 {
        /// The source map document.
        json: String,
        /// Name the map uses for the original file.
        source: String,
    },
    /// Volar-style offset tables, one entry per mapped interval.
    Volar {
        /// The mapping entries.
        mappings: Vec<VolarMapping>,
    },
}

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

/// Why a projection map could not be loaded or a query could not be answered.
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    /// The transport itself is malformed, or contradicts the sources it names.
    #[error("invalid projection map: {0}")]
    Map(
        /// Human-readable map validation failure.
        String,
    ),
    /// A host coordinate did not address a scalar boundary.
    #[error(transparent)]
    Coordinate(
        /// Coordinate conversion failure reported while reading or querying the map.
        #[from]
        CoordinateError,
    ),
    /// A span was not a valid slice of the source it claims to belong to.
    #[error(transparent)]
    Source(
        /// Source-buffer validation failure.
        #[from]
        SourceError,
    ),
    /// The map has no single original location for the span.
    #[error("projection has no unique original location for {0:?}")]
    Unmapped(
        /// Queried half-open UTF-8 span in the generated source.
        Span,
    ),
    /// The mapped original text differs from the projected text.
    #[error("projected text does not match original source at {0:?}")]
    Content(
        /// Mapped half-open UTF-8 span in the original source whose text did not match.
        Span,
    ),
}

/// A span in each source, kept together rather than changing its coordinate system.
///
/// Both halves are half-open spans in their own [`SourceFile`]; the original span
/// returned by exact verification contains the same text. Fragment verification may
/// omit synthetic gaps inside the projected envelope. Public construction and
/// deserialization do not validate this correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceProvenance {
    /// The span that was asked about, in the generated (shadow) source.
    pub projected: Span,
    /// The original (editable) span holding the same text.
    pub original: Span,
}

/// A generated/original pair of equal length with identical text.
///
/// Verified text equality allows interior byte offsets to be translated by addition.
#[derive(Clone, Debug)]
struct LinearRange {
    /// Half-open UTF-8 span of retained text in the generated source.
    generated: Span,
    /// Half-open UTF-8 span with identical text and byte length in the original source.
    original: Span,
}

/// A loaded projection map, queried by generated span.
///
/// The index keeps points and intervals separate: v3 points remain points and Volar
/// intervals remain intervals. We never infer an editable interval merely by
/// extrapolating a greatest-lower-bound token.
pub struct ProjectionIndex<'a> {
    /// Projected source whose spans are queried.
    generated: &'a SourceFile,
    /// Editable source against which mappings are validated.
    original: &'a SourceFile,
    /// Generated byte offset to original byte offset; None marks foreign-source or
    /// conflicting tokens. Missing keys are unmapped.
    points: BTreeMap<u32, Option<u32>>,
    /// Verified equal-text intervals sorted by generated start; overlaps are permitted
    /// and disagreement rejects a query.
    ranges: Vec<LinearRange>,
}

impl<'a> ProjectionIndex<'a> {
    /// Loads `map` and validates it against both sources.
    ///
    /// Rejects stale `sourcesContent`, mismatched Volar arrays, and malformed
    /// coordinates. Matching v3 tokens inside the same surrogate scalar are accepted
    /// but skipped: they cannot serve as byte-oriented edit anchors.
    ///
    /// # Errors
    ///
    /// Rejects malformed coordinates, stale contents, and invalid map structure.
    #[allow(clippy::too_many_lines)] // Keep the two transport formats and their validation together.
    pub fn new(
        generated: &'a SourceFile,
        original: &'a SourceFile,
        map: &ProjectionMap,
    ) -> Result<Self, ProjectionError> {
        let generated_index = CoordinateIndex::new(generated);
        let original_index = CoordinateIndex::new(original);
        let mut result = Self {
            generated,
            original,
            points: BTreeMap::new(),
            ranges: Vec::new(),
        };
        match map {
            ProjectionMap::V3 { json, source } => {
                let map = SourceMap::from_json_string(json)
                    .map_err(|error| ProjectionError::Map(error.to_string()))?;
                let sources: Vec<_> = map
                    .get_sources()
                    .enumerate()
                    .filter(|(_, name)| *name == source)
                    .collect();
                let [(source_id, _)] = sources.as_slice() else {
                    return Err(ProjectionError::Map(
                        "expected exactly one matching source".into(),
                    ));
                };
                let source_id = u32::try_from(*source_id)
                    .map_err(|_| ProjectionError::Map("too many sources".into()))?;
                if map
                    .get_source_content(source_id)
                    .is_some_and(|content| content != original.text())
                {
                    return Err(ProjectionError::Map("stale sourcesContent".into()));
                }
                for token in map.get_tokens() {
                    let generated_position = Position {
                        line: token.get_dst_line(),
                        utf16_column: token.get_dst_col(),
                    };
                    let original_position = Position {
                        line: token.get_src_line(),
                        utf16_column: token.get_src_col(),
                    };
                    // v3 permits UTF-16-unit tokens inside a surrogate pair. They are
                    // not byte boundaries and cannot be edit anchors. Accept only a
                    // matching pair of interiors; malformed coordinates still fail
                    // instead of being silently dropped.
                    if token.get_source_id() == Some(source_id)
                        && let Some(generated_scalar) =
                            surrogate_scalar(generated, &generated_index, generated_position)
                        && let Some(original_scalar) =
                            surrogate_scalar(original, &original_index, original_position)
                        && generated_scalar == original_scalar
                    {
                        continue;
                    }
                    let generated = generated_index.at_position(generated_position)?.0;
                    let original = if token.get_source_id() == Some(source_id) {
                        Some(
                            original_index
                                .at_position(Position {
                                    line: token.get_src_line(),
                                    utf16_column: token.get_src_col(),
                                })?
                                .0,
                        )
                    } else {
                        None
                    };
                    result
                        .points
                        .entry(generated)
                        .and_modify(|previous| {
                            if *previous != original {
                                *previous = None;
                            }
                        })
                        .or_insert(original);
                }
            }
            ProjectionMap::Volar { mappings } => {
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
                            let generated_span =
                                Span::new(generated.id(), generated_start, generated_end);
                            let original_span =
                                Span::new(original.id(), original_start, original_end);
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
                let starts = result
                    .points
                    .keys()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>();
                for (generated_end, original_end) in exclusive_ends {
                    if starts.contains(&generated_end)
                        || result.ranges.iter().any(|range| {
                            range.generated.start.0 <= generated_end
                                && generated_end < range.generated.end.0
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
            }
        }
        Ok(result)
    }

    /// Translates one generated byte offset to its original byte offset.
    ///
    /// Returns `None` unless the offset is a character boundary, is covered by
    /// at least one point or interval, and every mapping that covers it agrees. `None`
    /// means unmapped and must not be treated as the start of the file or as the
    /// nearest mapped neighbor.
    pub fn original_byte(&self, byte: BytePos) -> Option<BytePos> {
        if !self.generated.text().is_char_boundary(byte.0 as usize) {
            return None;
        }
        let mut found = self.points.get(&byte.0).copied();
        for range in self
            .ranges
            .iter()
            .take_while(|range| range.generated.start.0 <= byte.0)
        {
            if byte.0 < range.generated.end.0 {
                let mapped = range.original.start.0 + byte.0 - range.generated.start.0;
                if found.is_some_and(|value| value != Some(mapped)) {
                    return None;
                }
                found = Some(Some(mapped));
            }
        }
        found.flatten().map(BytePos)
    }

    /// True only when valid, nonempty syntax has no map entries or intervals. Ambiguous
    /// points count as entries, never as evidence of generated syntax.
    ///
    /// Callers use this to recognize scaffolding a converter inserted (a retyped
    /// type-alias header, an added reference list) and drop it from the analysis view
    /// without touching the original text.
    pub fn is_unmapped_syntax(&self, span: Span) -> bool {
        self.generated.slice(span).is_ok()
            && span.start < span.end
            && self.points.range(span.start.0..span.end.0).next().is_none()
            && !self
                .ranges
                .iter()
                .any(|range| range.generated.start < span.end && span.start < range.generated.end)
    }

    /// Verifies one projected span and returns where it came from.
    ///
    /// # Errors
    ///
    /// Requires mapped first/last scalars and exact original text. In particular, an
    /// exclusive-end token is not required (some converters move it).
    pub fn verify_exact(&self, projected: Span) -> Result<SourceProvenance, ProjectionError> {
        self.verify_fragments(&[projected])
    }

    /// Verifies a normalized expression assembled from ordered generated fragments.
    /// Omitted synthetic text never becomes part of the emitted original expression.
    ///
    /// # Errors
    ///
    /// Rejects invalid, overlapping, nonmonotonic, unmapped or text-mismatched
    /// fragments.
    pub fn verify_fragments(
        &self,
        fragments: &[Span],
    ) -> Result<SourceProvenance, ProjectionError> {
        let (Some(first), Some(last_span)) = (fragments.first(), fragments.last()) else {
            return Err(ProjectionError::Map("empty projection fragments".into()));
        };
        let projected = Span::new(self.generated.id(), first.start.0, last_span.end.0);
        let mut text = String::new();
        let mut previous_end = 0;
        for fragment in fragments {
            if fragment.start.0 < previous_end {
                return Err(ProjectionError::Unmapped(projected));
            }
            text.push_str(self.generated.slice(*fragment)?);
            previous_end = fragment.end.0;
        }
        let last_text = self.generated.slice(*last_span)?;
        let Some((last, character)) = last_text.char_indices().next_back() else {
            return Err(ProjectionError::Unmapped(projected));
        };
        let last = u32::try_from(last).map_err(|_| ProjectionError::Unmapped(projected))?;
        let width = u32::try_from(character.len_utf8())
            .map_err(|_| ProjectionError::Unmapped(projected))?;
        let start = self
            .original_byte(projected.start)
            .ok_or(ProjectionError::Unmapped(projected))?;
        let end = self
            .original_byte(BytePos(last_span.start.0 + last))
            .ok_or(ProjectionError::Unmapped(projected))?
            .0
            .checked_add(width)
            .ok_or(ProjectionError::Unmapped(projected))?;
        let original = Span::new(self.original.id(), start.0, end);
        if self.original.slice(original)? != text {
            return Err(ProjectionError::Content(original));
        }
        let mut previous = start.0;
        for (_, mapped) in fragments
            .iter()
            .flat_map(|fragment| self.points.range(fragment.start.0..fragment.end.0))
        {
            let Some(mapped) = mapped else {
                return Err(ProjectionError::Unmapped(projected));
            };
            if *mapped < previous || *mapped >= end {
                return Err(ProjectionError::Unmapped(projected));
            }
            previous = *mapped;
        }
        Ok(SourceProvenance {
            projected,
            original,
        })
    }

    /// Tests whether the converter collapsed a generated prefix onto the retained
    /// expression. Actual user-written receivers have distinct original positions.
    pub fn is_collapsed_prefix(&self, whole: Span, retained: Span) -> bool {
        if whole.file != self.generated.id()
            || retained.file != whole.file
            || whole.start.0 >= retained.start.0
            || whole.end != retained.end
        {
            return false;
        }
        let Ok(provenance) = self.verify_exact(retained) else {
            return false;
        };
        if self.verify_exact(whole).is_ok() {
            return false;
        }
        if self.ranges.iter().any(|range| {
            range.generated.start.0 < retained.start.0 && range.generated.end.0 > whole.start.0
        }) {
            return false;
        }
        self.points
            .range(whole.start.0..retained.start.0)
            .all(|(_, mapped)| mapped.is_none_or(|mapped| mapped == provenance.original.start.0))
    }

    /// Checks that all mapped pieces of a wrapper originate within the retained source
    /// range. This also supports duplicated identifiers in expanded shorthand
    /// properties without treating authored `key: value` as shorthand.
    pub fn is_collapsed_wrapper(&self, whole: Span, retained: Span) -> bool {
        if whole.file != self.generated.id()
            || retained.file != whole.file
            || whole.start.0 > retained.start.0
            || whole.end.0 < retained.end.0
        {
            return false;
        }
        let Ok(mapped) = self.verify_exact(retained) else {
            return false;
        };
        if self.verify_exact(whole).is_ok() {
            return false;
        }
        if !self
            .points
            .range(whole.start.0..whole.end.0)
            .all(|(_, point)| {
                point.is_none_or(|point| {
                    point >= mapped.original.start.0 && point <= mapped.original.end.0
                })
            })
        {
            return false;
        }
        self.ranges
            .iter()
            .filter(|range| {
                range.generated.start.0 < whole.end.0 && range.generated.end.0 > whole.start.0
            })
            .all(|range| {
                let start = range.original.start.0 + whole.start.0.max(range.generated.start.0)
                    - range.generated.start.0;
                let end = range.original.start.0 + whole.end.0.min(range.generated.end.0)
                    - range.generated.start.0;
                start >= mapped.original.start.0 && end <= mapped.original.end.0
            })
    }
}

/// Returns the scalar whose surrogate pair surrounds `position`.
///
/// Applies only when `position` is the interior UTF-16 unit of a pair in `source`,
/// which is a location no byte offset can name. Anything else returns `None`, so
/// callers treat it as a real coordinate rather than as text to skip.
fn surrogate_scalar<'a>(
    source: &'a SourceFile,
    index: &CoordinateIndex,
    position: Position,
) -> Option<&'a str> {
    let before = index
        .at_position(Position {
            utf16_column: position.utf16_column.checked_sub(1)?,
            ..position
        })
        .ok()?;
    let after = index
        .at_position(Position {
            utf16_column: position.utf16_column.checked_add(1)?,
            ..position
        })
        .ok()?;
    let text = source
        .slice(Span::new(source.id(), before.0, after.0))
        .ok()?;
    (text.chars().count() == 1 && text.encode_utf16().count() == 2).then_some(text)
}
