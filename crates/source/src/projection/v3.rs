//! Source map v3 loading with source-content and UTF-16 anchor validation.
//!
//! Tokens remain points; matching surrogate interiors cannot become edit anchors.

use std::collections::BTreeMap;

use lean_string::ToLeanStr;
use oxc_sourcemap::SourceMap;

use super::index::{ProjectionError, ProjectionIndex};
use crate::{CoordinateIndex, Position, SourceFile, Span};

/// Loads v3 points against both source buffers, rejecting malformed or stale maps.
pub(super) fn load<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    json: &str,
    source: &str,
) -> Result<ProjectionIndex<'a>, ProjectionError> {
    let generated_index = CoordinateIndex::new(generated);
    let original_index = CoordinateIndex::new(original);
    let mut result = ProjectionIndex {
        generated,
        original,
        points: BTreeMap::new(),
        ranges: Vec::new(),
    };
    let map = SourceMap::from_json_string(json)
        .map_err(|error| ProjectionError::Map(error.to_lean_str()))?;
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
    let source_id =
        u32::try_from(*source_id).map_err(|_| ProjectionError::Map("too many sources".into()))?;
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
    Ok(result)
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
