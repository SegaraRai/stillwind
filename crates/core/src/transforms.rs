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
pub struct PropsTransform {
    pub call_span: Span,
    pub replacement: String,
}

pub(crate) fn apply_transforms(
    source: &SourceFile,
    transforms: &[PropsTransform],
) -> Result<String, TransformError> {
    let mut transforms = transforms.iter().collect::<Vec<_>>();
    transforms.sort_by_key(|transform| std::cmp::Reverse(transform.call_span.start.0));

    let mut output = source.text().to_owned();
    let mut previous_start = u32::MAX;
    for transform in transforms {
        source
            .slice(transform.call_span)
            .map_err(TransformError::InvalidSpan)?;
        if transform.call_span.end.0 > previous_start {
            return Err(TransformError::Overlap {
                span: transform.call_span,
                previous_start,
            });
        }
        let start = transform.call_span.start.0 as usize;
        let end = transform.call_span.end.0 as usize;
        output.replace_range(start..end, &transform.replacement);
        previous_start = transform.call_span.start.0;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{PropsTransform, TransformError, apply_transforms};
    use std::error::Error;
    use stillwind_source::{FileId, SourceError, SourceFile, Span};

    fn replacements(spans: &[Span]) -> Vec<PropsTransform> {
        spans
            .iter()
            .map(|&call_span| PropsTransform {
                call_span,
                replacement: "x".into(),
            })
            .collect()
    }

    #[test]
    fn transform_errors_preserve_wrong_file_and_utf8_causes() {
        let source = SourceFile::new(FileId(1), "source", "éabc");
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
        let source = SourceFile::new(FileId(1), "source", "abcd");
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

    proptest::proptest! {
        #[test]
        fn replacing_one_span_preserves_both_sides(prefix in "[a-z]{0,12}", suffix in "[a-z]{0,12}", replacement in "[A-Z]{0,12}") {
            let source = SourceFile::new(FileId(1), "x", format!("{prefix}é{suffix}"));
            let start = u32::try_from(prefix.len()).unwrap();
            let transform = PropsTransform {
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
