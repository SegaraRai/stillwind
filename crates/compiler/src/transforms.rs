//! Source-spanned replacements shared by compiler output representations.

use serde::Serialize;

use stillwind_source::{SourceError, SourceFile, Span};

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("invalid stillwind transform span: {0}")]
    InvalidSpan(#[source] SourceError),
    #[error("overlapping stillwind transform span {span:?}, previous start {previous_start}")]
    Overlap { span: Span, previous_start: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceTransform {
    pub call_span: Span,
    pub replacement: String,
}

pub(crate) fn apply_transforms(
    source: &SourceFile,
    transforms: &[SourceTransform],
) -> Result<String, TransformError> {
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
    for transform in transforms.into_iter().rev() {
        let start = transform.call_span.start.0 as usize;
        let end = transform.call_span.end.0 as usize;
        output.push_str(&source.text()[cursor..start]);
        output.push_str(&transform.replacement);
        cursor = end;
    }
    output.push_str(&source.text()[cursor..]);
    Ok(output)
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
