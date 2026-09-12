//! Source-spanned replacements shared by compiler output representations.
//!
//! A [`SourceTransform`] replaces one span of the original source with generated text;
//! [`render_transforms`] applies a whole plan in one forward pass and reports both the
//! new code and the UTF-16 boundaries a source map needs. The plan is validated before
//! anything is emitted: every span must belong to the source file, and no two edits may
//! overlap or share a start offset, because an insertion at the same anchor as a
//! replacement has no defined order. Intervals no edit touches are copied verbatim, so
//! comments and user formatting survive exactly.

use serde::Serialize;

use stillwind_source::{SourceError, SourceFile, Span};

/// Reasons a transform plan cannot be applied.
#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    /// The compilation produced diagnostics, so its edits must not be applied.
    #[error("cannot render a failed compilation")]
    CompilationFailed,
    /// The span does not address a valid range of the given source file.
    #[error("invalid stillwind transform span: {0}")]
    InvalidSpan(#[source] SourceError),
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
/// `call_span` always addresses the original buffer, never previously generated text,
/// so edits are expressed against one stable coordinate space. An empty replacement
/// deletes the range.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceTransform {
    /// Range of the original source to replace; never a range of generated text.
    pub call_span: Span,
    /// Text to put in its place; empty means deletion.
    pub replacement: String,
}

/// UTF-16 replacement boundaries; untouched intervals retain exact correspondence.
/// One replaced range in both coordinate spaces, in UTF-16 code units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TransformMapping {
    /// Start of the replaced range, in UTF-16 code units of the original source.
    pub original_start: u32,
    /// End of the replaced range, exclusive.
    pub original_end: u32,
    /// Start of the replacement, in UTF-16 code units of the generated source.
    pub generated_start: u32,
    /// End of the replacement, exclusive.
    pub generated_end: u32,
}

/// Rendered source code with the boundaries of every replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedSource {
    /// The rendered source text.
    pub code: String,
    /// One entry per transform, in source order.
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

/// Validates all edits, then emits code and UTF-16 boundaries in one forward pass.
/// Work after sorting is linear in source, replacement text, and edit count.
///
/// # Errors
///
/// Rejects invalid spans, overlapping edits (including coincident insertions),
/// and output exceeding the 32-bit coordinate limit. No partial output is returned.
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

    fn replacements(spans: &[Span]) -> Vec<SourceTransform> {
        spans
            .iter()
            .map(|&call_span| SourceTransform {
                call_span,
                replacement: "x".into(),
            })
            .collect()
    }

    #[test]
    fn transform_errors_preserve_wrong_file_and_utf8_causes() {
        let source = SourceFile::new(FileId(1), "source", "éabc").unwrap();
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
        let source = SourceFile::new(FileId(1), "source", "abcd").unwrap();
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
        let source = SourceFile::new(FileId(1), "x", "🧭abc日本def\r\n").unwrap();
        let transforms = vec![
            SourceTransform {
                call_span: Span::new(FileId(1), 13, 16),
                replacement: "終".into(),
            },
            SourceTransform {
                call_span: Span::new(FileId(1), 4, 7),
                replacement: "開始".into(),
            },
        ];
        assert_eq!(
            apply_transforms(&source, &transforms).unwrap(),
            "🧭開始日本終\r\n"
        );
        let insertion = SourceTransform {
            call_span: Span::new(FileId(1), 4, 4),
            replacement: "import;".into(),
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
            let source = SourceFile::new(FileId(1), "component.svelte", &*text).unwrap();
            let first_start = prefix.len();
            let first_end = first_start + "日本".len();
            let second_start = first_end + middle.len();
            let second_end = second_start + "😀".len();
            let edits = vec![
                SourceTransform { call_span: Span::new(FileId(1), u32::try_from(second_start).unwrap(), u32::try_from(second_end).unwrap()), replacement: second.clone() },
                SourceTransform { call_span: Span::new(FileId(1), u32::try_from(first_start).unwrap(), u32::try_from(first_end).unwrap()), replacement: first.clone() },
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
            let source = SourceFile::new(FileId(1), "x", format!("{prefix}é{suffix}")).unwrap();
            let start = u32::try_from(prefix.len()).unwrap();
            let transform = SourceTransform {
                call_span: Span::new(FileId(1), start, start + 2),
                replacement: replacement.clone()
            };
            proptest::prop_assert_eq!(
                apply_transforms(&source, &[transform]).unwrap(),
                format!("{prefix}{replacement}{suffix}")
            );
        }
    }
}
