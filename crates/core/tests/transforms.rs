use std::error::Error;

use stillwind_core::{CompileResult, PropsTransform, TransformError};
use stillwind_source::{FileId, SourceError, SourceFile, Span};

fn result(spans: &[Span]) -> CompileResult {
    CompileResult {
        transforms: spans
            .iter()
            .map(|&call_span| PropsTransform {
                call_span,
                replacement: "x".into(),
            })
            .collect(),
        ..CompileResult::default()
    }
}

#[test]
fn transform_errors_preserve_wrong_file_and_utf8_causes() {
    let source = SourceFile::new(FileId(1), "source", "éabc");
    let error = result(&[Span::new(FileId(2), 0, 2)])
        .apply_to_source(&source)
        .unwrap_err();
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
            matches!(result(&[span]).apply_to_source(&source), Err(TransformError::InvalidSpan(SourceError::InvalidSpan(actual))) if actual == span)
        );
    }
}

#[test]
fn overlaps_are_distinguished_from_valid_adjacent_replacements() {
    let source = SourceFile::new(FileId(1), "source", "abcd");
    let overlap = Span::new(FileId(1), 0, 3);
    assert!(
        matches!(result(&[overlap, Span::new(FileId(1), 2, 4)]).apply_to_source(&source), Err(TransformError::Overlap { span, previous_start: 2 }) if span == overlap)
    );
    assert_eq!(
        result(&[Span::new(FileId(1), 0, 2), Span::new(FileId(1), 2, 4)])
            .apply_to_source(&source)
            .unwrap(),
        "xx"
    );
}
