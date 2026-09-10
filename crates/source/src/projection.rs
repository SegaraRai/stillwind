//! Validated correspondence from generated JavaScript to an original source.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    BytePos, CoordinateError, CoordinateIndex, Position, SourceError, SourceFile, Span, Utf16Offset,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectionMap {
    V3 { json: String, source: String },
    Volar { mappings: Vec<VolarMapping> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolarMapping {
    pub source_offsets: Vec<u32>,
    pub generated_offsets: Vec<u32>,
    pub lengths: Vec<u32>,
    #[serde(default)]
    pub generated_lengths: Option<Vec<u32>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("invalid projection map: {0}")]
    Map(String),
    #[error(transparent)]
    Coordinate(#[from] CoordinateError),
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error("projection has no unique original location for {0:?}")]
    Unmapped(Span),
    #[error("projected text does not match original source at {0:?}")]
    Content(Span),
}

/// A span in each source, kept together rather than changing its coordinate system.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub projected: Span,
    pub original: Span,
}

#[derive(Clone, Debug)]
struct LinearRange {
    generated: Span,
    original: Span,
}

/// v3 points remain points; Volar intervals remain intervals. We never infer an
/// editable interval merely by extrapolating a greatest-lower-bound token.
pub struct ProjectionIndex<'a> {
    generated: &'a SourceFile,
    original: &'a SourceFile,
    points: BTreeMap<u32, Option<u32>>,
    ranges: Vec<LinearRange>,
}

impl<'a> ProjectionIndex<'a> {
    /// # Errors
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
                let map = oxc_sourcemap::SourceMap::from_json_string(json)
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
                    // v3 permits UTF-16-unit tokens inside a surrogate pair. They
                    // are not byte boundaries and cannot be edit anchors. Accept
                    // only a matching pair of interiors; malformed coordinates
                    // still fail instead of being silently dropped.
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
                        for (g, o) in [
                            (generated_start, original_start),
                            (generated_end, original_end),
                        ] {
                            result
                                .points
                                .entry(g)
                                .and_modify(|previous| {
                                    if *previous != Some(o) {
                                        *previous = None;
                                    }
                                })
                                .or_insert(Some(o));
                        }
                    }
                }
                result.ranges.sort_by_key(|range| range.generated.start.0);
            }
        }
        Ok(result)
    }

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

    /// True only when valid, nonempty syntax has no map entries or intervals.
    /// Ambiguous points count as entries, never as evidence of generated syntax.
    pub fn is_unmapped_syntax(&self, span: Span) -> bool {
        self.generated.slice(span).is_ok()
            && span.start < span.end
            && self.points.range(span.start.0..span.end.0).next().is_none()
            && !self
                .ranges
                .iter()
                .any(|range| range.generated.start < span.end && span.start < range.generated.end)
    }

    /// # Errors
    /// Requires mapped first/last scalars and exact original text. In particular,
    /// an exclusive-end token is not required (some converters move it).
    pub fn verify_exact(&self, projected: Span) -> Result<SourceProvenance, ProjectionError> {
        self.verify_fragments(&[projected])
    }

    /// Verifies a normalized expression assembled from ordered generated fragments.
    /// Omitted synthetic text never becomes part of the emitted original expression.
    ///
    /// # Errors
    /// Rejects invalid, overlapping, nonmonotonic, unmapped or text-mismatched fragments.
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

    /// Checks that all mapped pieces of a wrapper originate within the retained
    /// source range. This also supports duplicated identifiers in expanded
    /// shorthand properties without treating authored `key: value` as shorthand.
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
