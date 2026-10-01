//! Source-spanned replacements shared by compiler output representations.
//!
//! [`SourceTransform`] replaces a range of the original buffer and retains exact or
//! anchored origins inside its replacement. [`render_transforms`] validates the plan,
//! applies edits, and returns UTF-16 intervals for host source-map construction.
//! Unedited text is copied verbatim; synthetic replacement gaps use the edit's anchor.
//! This module does not produce source-map v3 or compose framework/loader maps.

use serde::Serialize;

use stillwind_source::{CoordinateIndex, SourceError, SourceFile, Span};

/// Reasons a transform plan cannot be applied.
#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    /// The compilation produced diagnostics, so its edits must not be applied.
    #[error("cannot render a failed compilation")]
    CompilationFailed,
    /// The span does not address a valid range of the given source file.
    #[error("invalid stillwind transform span: {0}")]
    InvalidSpan(
        /// Source slicing failure, including a foreign file or invalid UTF-8 boundary.
        #[source]
        SourceError,
    ),
    /// Expression ranges are empty, overlap, are out of bounds, or falsely claim exact text.
    #[error("invalid expression mapping in replacement at {0:?}")]
    InvalidExpressionMapping(
        /// Original call span of the replacement containing the invalid expression mapping.
        Span,
    ),
    /// `span` overlaps the preceding edit or shares its start offset.
    #[error("overlapping stillwind transform span {span:?}, previous start {previous_start}")]
    Overlap {
        /// The edit that starts too early.
        span: Span,
        /// Start offset of the previously accepted edit.
        previous_start: u32,
    },
    /// The rendered source or one of its offsets does not fit in 32 bits.
    #[error("generated source exceeds the 32-bit coordinate limit")]
    TooLarge,
}

/// One replacement of an original source range.
///
/// All edits share one original coordinate space; application order never rebases
/// their source spans. Use `render_transforms` to validate and apply a complete plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTransform {
    /// Half-open UTF-8 range in the original source; an empty span denotes insertion.
    pub call_span: Span,
    /// UTF-8 replacement text; empty text deletes a nonempty source range.
    pub replacement: String,
    /// Nonempty, nonoverlapping intervals in ascending replacement-byte order.
    /// Offsets must be UTF-8 boundaries within `replacement`; all origins belong to
    /// the source being edited. Empty lists are omitted from serialized transforms.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expressions: Vec<ExpressionMapping>,
}

impl SourceTransform {
    /// Rewrites each unmapped text interval while retaining mapped values verbatim.
    ///
    /// Requires valid ordered expression ranges. Replaces `replacement` and rebases
    /// generated offsets without changing origins or exactness. With no mappings,
    /// the callback receives the whole replacement; empty gaps may also be passed.
    ///
    /// # Panics
    ///
    /// Panics when an expression range starts before the previous range ended, extends
    /// past `replacement`, or falls inside a multi-byte scalar, because each range is
    /// sliced directly; a panicking `rewrite` propagates as well.
    ///
    /// Callers must build the ranges together with the replacement text they address -
    /// the compiler does this while lowering one macro call - and must not attach ranges
    /// carried over from another compilation. Mutation is not transactional: the
    /// replacement has been taken even when a panic starts.
    pub(crate) fn rewrite_generated(&mut self, rewrite: impl Fn(&str) -> String) {
        let old = std::mem::take(&mut self.replacement);
        let mut cursor = 0;
        for mapping in &mut self.expressions {
            self.replacement
                .push_str(&rewrite(&old[cursor..mapping.start]));
            cursor = mapping.end;
            let value = &old[mapping.start..mapping.end];
            mapping.start = self.replacement.len();
            self.replacement.push_str(value);
            mapping.end = self.replacement.len();
        }
        self.replacement.push_str(&rewrite(&old[cursor..]));
    }

    /// Replaces an attribute delimiter throughout the generated replacement.
    ///
    /// Requires valid ordered mappings and a nonempty entity spelling. Exact intervals
    /// must equal their original slices. Intervals containing the delimiter are split
    /// by scalar; each expanded entity becomes an anchored mapping to its original
    /// character, and unaffected intervals retain exactness. All generated offsets
    /// are rebased; synthetic text is encoded without acquiring source mappings.
    ///
    /// # Panics
    ///
    /// Panics when a mapping range starts before the previous range ended, extends past
    /// `replacement`, or falls inside a multi-byte scalar, because each range is sliced
    /// directly; an exact mapping whose original span does not cover the text it claims
    /// overflows the 32-bit source offset.
    ///
    /// Callers must pass the ranges that [`render_transforms`] validates against this
    /// same replacement, produced together with the generated text by the compiler;
    /// ranges from another compilation cannot be used. The replacement may already be
    /// partially rewritten when a panic starts.
    #[allow(clippy::cast_possible_truncation)] // Source-owned byte offsets fit in u32; scalars occupy at most four bytes.
    pub(crate) fn encode_attribute(&mut self, character: char, entity: &str) {
        let old = std::mem::take(&mut self.replacement);
        let mut cursor = 0;
        let mut mappings = Vec::new();
        for mapping in &self.expressions {
            self.replacement
                .push_str(&old[cursor..mapping.start].replace(character, entity));
            let value = &old[mapping.start..mapping.end];
            if !value.contains(character) {
                let start = self.replacement.len();
                self.replacement.push_str(value);
                mappings.push(ExpressionMapping {
                    start,
                    end: self.replacement.len(),
                    ..mapping.clone()
                });
                cursor = mapping.end;
                continue;
            }
            for (offset, scalar) in value.char_indices() {
                let start = self.replacement.len();
                if scalar == character {
                    self.replacement.push_str(entity);
                } else {
                    self.replacement.push(scalar);
                }
                let original = if mapping.exact {
                    // All offsets originate in a SourceFile, whose size fits in u32.
                    let start = mapping.original.start.0 + offset as u32;
                    Span::new(
                        mapping.original.file,
                        start,
                        start + scalar.len_utf8() as u32,
                    )
                } else {
                    mapping.original
                };
                mappings.push(ExpressionMapping {
                    start,
                    end: self.replacement.len(),
                    original,
                    exact: mapping.exact && scalar != character,
                });
            }
            cursor = mapping.end;
        }
        self.replacement
            .push_str(&old[cursor..].replace(character, entity));
        self.expressions = mappings;
    }
}

/// Correspondence for one retained occurrence inside a generated replacement.
/// The owner orders these by generated position; source order need not match.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressionMapping {
    /// Inclusive UTF-8 byte offset in the replacement; a scalar boundary less than `end`.
    pub start: usize,
    /// Exclusive UTF-8 byte offset in the replacement; a scalar boundary within its length.
    pub end: usize,
    /// Valid original-source UTF-8 span; distinct occurrences may contain identical text.
    pub original: Span,
    /// `true` requires identical generated and original text. `false` maps the whole
    /// generated interval to the original start, as used for an encoded delimiter.
    pub exact: bool,
}

/// One applied edit in original and generated coordinates for host map construction.
/// Intervals between edits remain exact; replacement gaps without expression mappings
/// use `original_start` as their fallback location.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformMapping {
    /// Inclusive absolute UTF-16 offset in the original source; equals end for insertion.
    pub original_start: u32,
    /// Exclusive absolute UTF-16 offset in the original source.
    pub original_end: u32,
    /// Inclusive absolute UTF-16 offset in rendered code, before host insertions/removals.
    pub generated_start: u32,
    /// Exclusive absolute UTF-16 offset in rendered code; equals start for deletion.
    pub generated_end: u32,
    /// Nonoverlapping expression intervals ordered by generated position and contained
    /// in this replacement. Original positions may move backwards or repeat.
    pub expressions: Vec<ExpressionTransformMapping>,
}

/// A retained expression's source correspondence after byte-to-UTF-16 conversion.
/// Used inside a replacement mapping to distinguish exact text from encoded anchors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressionTransformMapping {
    /// Inclusive absolute UTF-16 offset in the original buffer.
    pub original_start: u32,
    /// Exclusive absolute UTF-16 offset in the original buffer.
    pub original_end: u32,
    /// Inclusive absolute generated UTF-16 offset.
    pub generated_start: u32,
    /// Exclusive absolute generated UTF-16 offset.
    pub generated_end: u32,
    /// `true` advances both buffers by the same code-unit offset within the interval;
    /// `false` anchors every generated code unit at `original_start`.
    pub exact: bool,
}

/// Applied source edits and the correspondence needed to map their resulting code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedSource {
    /// Complete UTF-8 source after all edits, without host-added prefixes or removals.
    pub code: String,
    /// One entry per edit, ordered by both original and generated replacement start.
    pub mappings: Vec<TransformMapping>,
}

/// Applies `transforms` and returns only the rendered code.
///
/// # Errors
///
/// Propagates every failure of [`render_transforms`].
pub(crate) fn apply_transforms(
    source: &SourceFile,
    transforms: &[SourceTransform],
) -> Result<String, TransformError> {
    render_transforms(source, transforms).map(|output| output.code)
}

/// Applies edits against `source` and returns code with absolute UTF-16 mappings.
///
/// Input edits may be unsorted. The result orders them by source position, copies
/// untouched intervals, and validates expression correspondence while emitting each
/// replacement. Inputs are unchanged. Source offsets use one coordinate index;
/// expression lookups use binary search rather than rescanning source prefixes.
///
/// # Errors
///
/// Rejects foreign or invalid source spans, overlapping edits or shared edit starts,
/// invalid expression intervals, unequal text marked exact, and output beyond the
/// 32-bit coordinate limit. No partial output is returned.
pub fn render_transforms(
    source: &SourceFile,
    transforms: &[SourceTransform],
) -> Result<RenderedSource, TransformError> {
    let mut transforms = transforms.iter().collect::<Vec<_>>();
    transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

    let mut previous_start = u32::MAX;
    for transform in &transforms {
        source
            .slice(transform.call_span)
            .map_err(TransformError::InvalidSpan)?;
        if transform.call_span.end.0 > previous_start
            || transform.call_span.start.0 == previous_start
        {
            return Err(TransformError::Overlap {
                span: transform.call_span,
                previous_start,
            });
        }
        previous_start = transform.call_span.start.0;
    }
    let coordinates = CoordinateIndex::new(source);
    let mut output = String::with_capacity(source.text().len());
    let mut cursor = 0;
    let mut original_utf16 = 0_u32;
    let mut generated_utf16 = 0_u32;
    let mut mappings = Vec::with_capacity(transforms.len());
    for transform in transforms.into_iter().rev() {
        let start = transform.call_span.start.0 as usize;
        let end = transform.call_span.end.0 as usize;
        let gap = &source.text()[cursor..start];
        let gap_length = utf16_length(gap)?;
        original_utf16 = add_offset(original_utf16, gap_length)?;
        generated_utf16 = add_offset(generated_utf16, gap_length)?;
        let original_end = add_offset(original_utf16, utf16_length(&source.text()[start..end])?)?;
        let generated_end = add_offset(generated_utf16, utf16_length(&transform.replacement)?)?;
        mappings.push(TransformMapping {
            original_start: original_utf16,
            original_end,
            generated_start: generated_utf16,
            generated_end,
            expressions: expression_mappings(source, &coordinates, transform, generated_utf16)?,
        });
        output.push_str(gap);
        output.push_str(&transform.replacement);
        original_utf16 = original_end;
        generated_utf16 = generated_end;
        cursor = end;
    }
    output.push_str(&source.text()[cursor..]);
    u32::try_from(output.len()).map_err(|_| TransformError::TooLarge)?;
    Ok(RenderedSource {
        code: output,
        mappings,
    })
}

/// Validates one replacement's expression ranges and converts them to absolute UTF-16.
/// `coordinates` must index `source`; `generated_start` is the replacement's absolute
/// UTF-16 start. Returned intervals follow generated order, regardless of source order.
///
/// # Errors
///
/// Rejects invalid original spans, empty/overlapping/non-boundary generated ranges,
/// unequal text marked exact, and UTF-16 offset overflow.
fn expression_mappings(
    source: &SourceFile,
    coordinates: &CoordinateIndex,
    transform: &SourceTransform,
    generated_start: u32,
) -> Result<Vec<ExpressionTransformMapping>, TransformError> {
    let mut cursor = 0;
    let mut generated = generated_start;
    let mut result = Vec::new();
    for expression in &transform.expressions {
        let invalid = || TransformError::InvalidExpressionMapping(transform.call_span);
        let original = source
            .slice(expression.original)
            .map_err(TransformError::InvalidSpan)?;
        if expression.start < cursor || expression.start >= expression.end {
            return Err(invalid());
        }
        let prefix = transform
            .replacement
            .get(cursor..expression.start)
            .ok_or_else(invalid)?;
        let value = transform
            .replacement
            .get(expression.start..expression.end)
            .ok_or_else(invalid)?;
        if expression.exact && value != original {
            return Err(invalid());
        }
        generated = add_offset(generated, utf16_length(prefix)?)?;
        let end = add_offset(generated, utf16_length(value)?)?;
        result.push(ExpressionTransformMapping {
            original_start: coordinates
                .utf16(expression.original.start)
                .map_err(|_| invalid())?
                .0,
            original_end: coordinates
                .utf16(expression.original.end)
                .map_err(|_| invalid())?
                .0,
            generated_start: generated,
            generated_end: end,
            exact: expression.exact,
        });
        cursor = expression.end;
        generated = end;
    }
    Ok(result)
}

/// Counts UTF-16 code units without allocating an encoded copy.
///
/// # Errors
///
/// Returns `TooLarge` when the count cannot fit in `u32`.
fn utf16_length(text: &str) -> Result<u32, TransformError> {
    u32::try_from(text.encode_utf16().count()).map_err(|_| TransformError::TooLarge)
}

/// Adds two UTF-16 offsets, rejecting a sum that no longer fits the 32-bit coordinate space.
fn add_offset(offset: u32, length: u32) -> Result<u32, TransformError> {
    offset.checked_add(length).ok_or(TransformError::TooLarge)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use stillwind_source::{FileId, SourceError, SourceFile, Span};

    use super::*;

    /// Builds synthetic one-character replacements without expression-level correspondence.
    fn replacements(spans: &[Span]) -> Vec<SourceTransform> {
        spans
            .iter()
            .map(|&call_span| SourceTransform {
                call_span,
                replacement: "x".into(),
                expressions: Vec::new(),
            })
            .collect()
    }

    /// Exercises prefix expansion, unchanged authored prefixes, surrogate pairs, and encoded quotes.
    #[test]
    fn encoded_characters_and_synthetic_rewrites_keep_expression_origins() {
        let source = SourceFile::new(FileId(1), None, None, "\"😀--sw-user\"").unwrap();
        let mut edit = SourceTransform {
            call_span: source.full_span(),
            replacement: format!("--sw-generated({})", source.text()),
            expressions: vec![ExpressionMapping {
                start: 15,
                end: 15 + source.text().len(),
                original: source.full_span(),
                exact: true,
            }],
        };
        edit.rewrite_generated(|value| value.replace("--sw-", "--long-prefix-"));
        assert!(edit.replacement.starts_with("--long-prefix-generated("));
        assert!(edit.replacement.contains("--sw-user"));
        edit.encode_attribute('"', "&quot;");
        let rendered = render_transforms(&source, &[edit]).unwrap();
        let mappings = &rendered.mappings[0].expressions;
        assert_eq!(mappings.iter().filter(|mapping| !mapping.exact).count(), 2);
        let generated = rendered.code.encode_utf16().collect::<Vec<_>>();
        let original = source.text().encode_utf16().collect::<Vec<_>>();
        for mapping in mappings {
            let value =
                &generated[mapping.generated_start as usize..mapping.generated_end as usize];
            if mapping.exact {
                assert_eq!(
                    value,
                    &original[mapping.original_start as usize..mapping.original_end as usize]
                );
            } else {
                assert_eq!(String::from_utf16(value).unwrap(), "&quot;");
                assert_eq!(original[mapping.original_start as usize], u16::from(b'"'));
            }
        }
    }

    /// Rejects duplicate, reversed, out-of-bounds, and falsely exact expression intervals.
    #[test]
    fn invalid_expression_ranges_are_rejected() {
        let source = SourceFile::new(FileId(1), None, None, "abc").unwrap();
        let mapping = ExpressionMapping {
            start: 0,
            end: 3,
            original: source.full_span(),
            exact: true,
        };
        for expressions in [
            vec![mapping.clone(), mapping.clone()],
            vec![ExpressionMapping {
                start: 2,
                end: 1,
                ..mapping.clone()
            }],
            vec![ExpressionMapping {
                end: 4,
                ..mapping.clone()
            }],
            vec![ExpressionMapping {
                original: Span::new(source.id(), 0, 1),
                ..mapping
            }],
        ] {
            let edit = SourceTransform {
                call_span: source.full_span(),
                replacement: "abc".into(),
                expressions,
            };
            assert!(matches!(
                render_transforms(&source, &[edit]),
                Err(TransformError::InvalidExpressionMapping(_))
            ));
        }
    }

    #[test]
    fn transform_errors_preserve_wrong_file_and_utf8_causes() {
        let source = SourceFile::new(FileId(1), None, None, "éabc").unwrap();
        let error =
            apply_transforms(&source, &replacements(&[Span::new(FileId(2), 0, 2)])).unwrap_err();
        assert!(matches!(
            error,
            TransformError::InvalidSpan(SourceError::WrongFile {
                expected: FileId(1),
                actual: FileId(2)
            })
        ));
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<SourceError>()
                .is_some()
        );
        for span in [Span::new(FileId(1), 1, 2), Span::new(FileId(1), 0, 99)] {
            assert!(
                matches!(apply_transforms(&source, &replacements(&[span])), Err(TransformError::InvalidSpan(SourceError::InvalidSpan(actual))) if actual == span)
            );
        }
    }

    #[test]
    fn overlaps_are_distinguished_from_valid_adjacent_replacements() {
        let source = SourceFile::new(FileId(1), None, None, "abcd").unwrap();
        let overlap = Span::new(FileId(1), 0, 3);
        assert!(
            matches!(apply_transforms(&source, &replacements(&[overlap, Span::new(FileId(1), 2, 4)])), Err(TransformError::Overlap { span, previous_start: 2 }) if span == overlap)
        );
        assert_eq!(
            apply_transforms(
                &source,
                &replacements(&[Span::new(FileId(1), 0, 2), Span::new(FileId(1), 2, 4)])
            )
            .unwrap(),
            "xx"
        );
    }

    #[test]
    fn linear_render_preserves_unicode_gaps_and_rejects_same_anchor_collisions() {
        let source = SourceFile::new(FileId(1), None, None, "🧭abc日本def\r\n").unwrap();
        let transforms = vec![
            SourceTransform {
                call_span: Span::new(FileId(1), 13, 16),
                replacement: "終".into(),
                expressions: Vec::new(),
            },
            SourceTransform {
                call_span: Span::new(FileId(1), 4, 7),
                replacement: "開始".into(),
                expressions: Vec::new(),
            },
        ];
        assert_eq!(
            apply_transforms(&source, &transforms).unwrap(),
            "🧭開始日本終\r\n"
        );
        let insertion = SourceTransform {
            call_span: Span::new(FileId(1), 4, 4),
            replacement: "import;".into(),
            expressions: Vec::new(),
        };
        for edits in [
            vec![insertion.clone(), transforms[1].clone()],
            vec![transforms[1].clone(), insertion.clone()],
            vec![insertion.clone(), insertion],
        ] {
            assert!(matches!(
                apply_transforms(&source, &edits),
                Err(TransformError::Overlap { .. })
            ));
        }
    }

    proptest::proptest! {
        #[test]
        fn mapped_render_matches_independent_utf16_offsets(
            prefix in ".{0,30}", middle in ".{0,30}", suffix in ".{0,30}",
            first in ".{0,20}", second in ".{0,20}"
        ) {
            let text = format!("{prefix}日本{middle}😀{suffix}");
            let source = SourceFile::new(FileId(1), None, None, &*text).unwrap();
            let first_start = prefix.len();
            let first_end = first_start + "日本".len();
            let second_start = first_end + middle.len();
            let second_end = second_start + "😀".len();
            let edits = vec![
                SourceTransform {
                    call_span: Span::new(FileId(1), u32::try_from(second_start).unwrap(), u32::try_from(second_end).unwrap()),
                    replacement: second.clone(),
                    expressions: Vec::new(),
                },
                SourceTransform {
                    call_span: Span::new(FileId(1), u32::try_from(first_start).unwrap(), u32::try_from(first_end).unwrap()),
                    replacement: first.clone(),
                    expressions: Vec::new(),
                },
            ];
            let rendered = render_transforms(&source, &edits).unwrap();
            proptest::prop_assert_eq!(&rendered.code, &format!("{prefix}{first}{middle}{second}{suffix}"));
            for (mapping, (start, end, generated_prefix, replacement)) in rendered.mappings.iter().zip([
                (first_start, first_end, prefix.clone(), first.clone()),
                (second_start, second_end, format!("{prefix}{first}{middle}"), second.clone()),
            ]) {
                proptest::prop_assert_eq!(mapping.original_start as usize, text[..start].encode_utf16().count());
                proptest::prop_assert_eq!(mapping.original_end as usize, text[..end].encode_utf16().count());
                proptest::prop_assert_eq!(mapping.generated_start as usize, generated_prefix.encode_utf16().count());
                proptest::prop_assert_eq!(mapping.generated_end as usize, format!("{generated_prefix}{replacement}").encode_utf16().count());
            }
        }

        #[test]
        fn replacing_one_span_preserves_both_sides(prefix in "[a-z]{0,12}", suffix in "[a-z]{0,12}", replacement in "[A-Z]{0,12}") {
            let source = SourceFile::new(FileId(1), None, None, format!("{prefix}é{suffix}")).unwrap();
            let start = u32::try_from(prefix.len()).unwrap();
            let transform = SourceTransform {
                call_span: Span::new(FileId(1), start, start + 2),
                replacement: replacement.clone(),
                expressions: Vec::new(),
            };
            proptest::prop_assert_eq!(
                apply_transforms(&source, &[transform]).unwrap(),
                format!("{prefix}{replacement}{suffix}")
            );
        }
    }
}
