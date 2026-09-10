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
                    let generated = generated_index
                        .at_position(Position {
                            line: token.get_dst_line(),
                            utf16_column: token.get_dst_col(),
                        })?
                        .0;
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

    /// # Errors
    /// Requires mapped first/last scalars and exact original text. In particular,
    /// an exclusive-end token is not required (some converters move it).
    pub fn verify_exact(&self, projected: Span) -> Result<SourceProvenance, ProjectionError> {
        let text = self.generated.slice(projected)?;
        let Some((last, character)) = text.char_indices().next_back() else {
            return Err(ProjectionError::Unmapped(projected));
        };
        let last = u32::try_from(last).map_err(|_| ProjectionError::Unmapped(projected))?;
        let width = u32::try_from(character.len_utf8())
            .map_err(|_| ProjectionError::Unmapped(projected))?;
        let start = self
            .original_byte(projected.start)
            .ok_or(ProjectionError::Unmapped(projected))?;
        let end = self
            .original_byte(BytePos(projected.start.0 + last))
            .ok_or(ProjectionError::Unmapped(projected))?
            .0
            .checked_add(width)
            .ok_or(ProjectionError::Unmapped(projected))?;
        let original = Span::new(self.original.id(), start.0, end);
        if self.original.slice(original)? != text {
            return Err(ProjectionError::Content(original));
        }
        let mut previous = start.0;
        for (_, mapped) in self.points.range(projected.start.0..projected.end.0) {
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
}
