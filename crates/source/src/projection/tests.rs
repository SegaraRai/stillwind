//! Behaviour of the two transport formats a host converter produces.
//!
//! Each test builds a real map (v3 through a source map builder, Volar through
//! hand-written offset tables), loads it against a generated and an original
//! source file, and asserts the accept and reject behaviour callers rely on:
//! exact-text verification, ambiguity rejection, file identity, and surrogate handling.

use oxc_sourcemap::SourceMapBuilder;

use super::*;
use crate::{BytePos, FileId, SourceFile, Span};

/// Builds a v3 map whose tokens all name the original as their source, one token per
/// (generated, original) UTF-16 pair. A `None` original emits an unmapped token, so
/// tests can mix mapped and unmapped positions in one map.
fn point_map(points: &[(u32, Option<u32>)], original: &str) -> ProjectionMap {
    let mut builder = SourceMapBuilder::default();
    let source = builder.set_source_and_content("component", original);
    for &(generated, original) in points {
        builder.add_token(
            0,
            generated,
            0,
            original.unwrap_or(0),
            original.map(|_| source),
            None,
        );
    }
    ProjectionMap::V3 {
        json: builder.into_sourcemap().to_json_string(),
        source: "component".into(),
    }
}

/// A span needs both a mapped first and a mapped last scalar, and the result keeps the
/// original file id rather than reusing the generated one.
#[test]
fn exact_range_requires_two_anchors_and_preserves_file_identity() {
    let original = SourceFile::new(FileId(1), "component", "<x>style('flex')</x>").unwrap();
    let generated = SourceFile::new(FileId(2), "component.ts", "style('flex');").unwrap();
    let span = Span::new(FileId(2), 0, 13);
    let map = point_map(
        &[(0, Some(3)), (12, Some(15)), (13, Some(0))],
        original.text(),
    );
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(
        index.verify_exact(span).unwrap().original,
        Span::new(FileId(1), 3, 16)
    );
    assert!(index.verify_exact(Span::new(FileId(1), 0, 13)).is_err());
    let start_only = point_map(&[(0, Some(3))], original.text());
    assert!(
        ProjectionIndex::new(&generated, &original, &start_only)
            .unwrap()
            .verify_exact(span)
            .is_err()
    );
}

/// Interior gaps, disagreeing duplicate tokens and non-monotonic mappings all fail
/// verification, and sourcesContent that no longer matches the original is a
/// construction error rather than a silently ignored field.
#[test]
fn refuses_unmapped_interior_conflicting_duplicates_and_wrong_contents() {
    let original = SourceFile::new(FileId(1), "component", "abcdabcd").unwrap();
    let generated = SourceFile::new(FileId(2), "component.ts", "abcd").unwrap();
    for points in [
        vec![(0, Some(0)), (1, None), (3, Some(3))],
        vec![(0, Some(0)), (0, Some(4)), (3, Some(3))],
        vec![(0, Some(0)), (2, Some(6)), (3, Some(3))],
        vec![(0, Some(1)), (3, Some(4))],
    ] {
        let map = point_map(&points, original.text());
        assert!(
            ProjectionIndex::new(&generated, &original, &map)
                .unwrap()
                .verify_exact(generated.full_span())
                .is_err()
        );
    }
    let stale = point_map(&[(0, Some(0))], "stale");
    assert!(ProjectionIndex::new(&generated, &original, &stale).is_err());
}

/// Volar offsets and lengths count UTF-16 units, so a multi-byte original maps
/// correctly; ragged arrays and a length that cuts a surrogate pair are rejected at
/// construction.
#[test]
fn volar_intervals_use_utf16_and_reject_ambiguous_or_malformed_ranges() {
    let original = SourceFile::new(FileId(1), "component", "<x>日本😀</x>").unwrap();
    let generated = SourceFile::new(FileId(2), "component.ts", "日本😀").unwrap();
    let mapping = VolarMapping {
        source_offsets: vec![3],
        generated_offsets: vec![0],
        lengths: vec![4],
        generated_lengths: None,
    };
    let map = ProjectionMap::Volar {
        mappings: vec![mapping.clone()],
    };
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(
        index.verify_exact(generated.full_span()).unwrap().original,
        Span::new(FileId(1), 3, 13)
    );
    let malformed = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            lengths: vec![],
            ..mapping.clone()
        }],
    };
    assert!(ProjectionIndex::new(&generated, &original, &malformed).is_err());
    let surrogate = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            lengths: vec![3],
            ..mapping
        }],
    };
    assert!(ProjectionIndex::new(&generated, &original, &surrogate).is_err());
}

/// Adjacent intervals that a converter reordered keep their own start anchors, and
/// overlapping intervals pointing at different originals make the offset ambiguous,
/// which must resolve to nothing instead of to a best guess.
#[test]
fn adjacent_reordered_volar_ranges_keep_their_own_start_anchors() {
    let original = SourceFile::new(FileId(1), "component", "secondfirst").unwrap();
    let generated = SourceFile::new(FileId(2), "component.ts", "firstsecond").unwrap();
    let map = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 6, 0, 11],
            generated_offsets: vec![0, 0, 5, 7],
            lengths: vec![0, 5, 6, 0],
            generated_lengths: None,
        }],
    };
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(
        index
            .verify_exact(Span::new(FileId(2), 0, 5))
            .unwrap()
            .original,
        Span::new(FileId(1), 6, 11)
    );
    assert_eq!(
        index
            .verify_exact(Span::new(FileId(2), 5, 11))
            .unwrap()
            .original,
        Span::new(FileId(1), 0, 6)
    );
    assert!(index.verify_exact(generated.full_span()).is_err());
    let ambiguous = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 6],
            generated_offsets: vec![0, 0],
            lengths: vec![1, 1],
            generated_lengths: None,
        }],
    };
    assert!(
        ProjectionIndex::new(&generated, &original, &ambiguous)
            .unwrap()
            .original_byte(BytePos(0))
            .is_none()
    );
}

/// A v3 token sitting inside a surrogate pair is skipped rather than turned into a byte
/// offset, while an interior token with no matching counterpart is still reported as a
/// malformed map.
#[test]
fn v3_surrogate_unit_tokens_are_not_edit_boundaries() {
    let original = SourceFile::new(FileId(1), "component", "x😀y").unwrap();
    let generated = SourceFile::new(FileId(2), "component.ts", "😀").unwrap();
    let map = point_map(&[(0, Some(1)), (1, Some(2))], original.text());
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(
        index.verify_exact(generated.full_span()).unwrap().original,
        Span::new(FileId(1), 1, 5)
    );
    assert!(index.original_byte(BytePos(1)).is_none());
    let malformed = point_map(&[(0, Some(1)), (1, Some(3))], original.text());
    assert!(ProjectionIndex::new(&generated, &original, &malformed).is_err());
}

/// An empty span verifies only at a boundary both sides agree on.
#[test]
fn empty_spans_require_an_exact_mapped_boundary() {
    let original = SourceFile::new(FileId(1), "component", "<x>''</x>").unwrap();
    let generated = SourceFile::new(FileId(2), "component.ts", "''").unwrap();
    let map = point_map(&[(1, Some(4))], original.text());
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(
        index
            .verify_exact(Span::new(FileId(2), 1, 1))
            .unwrap()
            .original,
        Span::new(FileId(1), 4, 4)
    );
    assert!(index.verify_exact(Span::new(FileId(2), 0, 0)).is_err());
}

/// Overlapping intervals that agree on the text they cover stay usable: an interior
/// position resolves, and the whole span verifies.
#[test]
fn agreeing_overlapping_projection_ranges_are_accepted() {
    let original = SourceFile::new(FileId(1), "original", "abcd").unwrap();
    let generated = SourceFile::new(FileId(2), "generated", "abcd").unwrap();
    let map = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 1],
            generated_offsets: vec![0, 1],
            lengths: vec![3, 3],
            generated_lengths: None,
        }],
    };
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(index.original_byte(BytePos(2)), Some(BytePos(2)));
    assert_eq!(
        index.verify_exact(generated.full_span()).unwrap().original,
        original.full_span()
    );
}
