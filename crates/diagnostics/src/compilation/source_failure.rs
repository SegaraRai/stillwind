//! Source and projection conversions retain rejected coordinates and their operation.

use serde::{Deserialize, Serialize};

use stillwind_source::{CoordinateError, ProjectionError, SourceError};

use super::CompileDiagnostic;

/// Operation that encountered a source-coordinate failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceFailureContext {
    /// Utility tokenization.
    Utility,
    /// Framework projection.
    Projection,
    /// CSS source loading.
    Stylesheet,
    /// Style call emission.
    Emission,
    /// JavaScript source extraction.
    Frontend,
    /// Source-transform rendering.
    Transform,
    /// Macro import removal and runtime import insertion.
    MacroImport,
}

impl CompileDiagnostic {
    /// Preserves source-failure fields and the operation supplied by `context`.
    /// This does not validate coordinates or attach a primary reporting location.
    pub fn source_failure(error: &SourceError, context: SourceFailureContext) -> Self {
        match *error {
            SourceError::InternalSourceMap => Self::SourceInternalMap { context },
            SourceError::TooLarge(value) => Self::SourceTooLarge { value, context },
            SourceError::InvalidRange { start, end } => Self::SourceInvalidRange {
                start,
                end,
                context,
            },
            SourceError::WrongFile { expected, actual } => Self::SourceWrongFile {
                expected,
                actual,
                context,
            },
            SourceError::InvalidSpan(span) => Self::SourceInvalidSpan { span, context },
            SourceError::InvalidOffset(offset) => Self::SourceInvalidOffset { offset, context },
        }
    }
}

impl From<ProjectionError> for CompileDiagnostic {
    fn from(error: ProjectionError) -> Self {
        match error {
            ProjectionError::Map(detail) => Self::ProjectionMap { detail },
            ProjectionError::Source(error) => {
                Self::source_failure(&error, SourceFailureContext::Projection)
            }
            ProjectionError::Coordinate(error) => match error {
                CoordinateError::Offset(offset) => Self::InvalidUtf16Offset { offset },
                CoordinateError::Position(position) => Self::InvalidUtf16Position { position },
                CoordinateError::Byte(offset) => Self::InvalidUtf8Offset { offset },
            },
            ProjectionError::Unmapped(span) => Self::ProjectionUnmapped { span },
            ProjectionError::Content(span) => Self::ProjectionContent { span },
        }
    }
}

impl CompileDiagnostic {
    /// Returns whether a projection or its coordinates are invalid.
    pub const fn is_projection_failure(&self) -> bool {
        matches!(
            self,
            Self::ProjectionMap { .. }
                | Self::ProjectionSyntax { .. }
                | Self::ProjectionPreparation { .. }
                | Self::ProjectionUnmapped { .. }
                | Self::ProjectionContent { .. }
                | Self::InvalidUtf16Offset { .. }
                | Self::InvalidUtf16Position { .. }
                | Self::InvalidUtf8Offset { .. }
                | Self::SourceInternalMap {
                    context: SourceFailureContext::Projection,
                    ..
                }
                | Self::SourceTooLarge {
                    context: SourceFailureContext::Projection,
                    ..
                }
                | Self::SourceInvalidRange {
                    context: SourceFailureContext::Projection,
                    ..
                }
                | Self::SourceWrongFile {
                    context: SourceFailureContext::Projection,
                    ..
                }
                | Self::SourceInvalidSpan {
                    context: SourceFailureContext::Projection,
                    ..
                }
                | Self::SourceInvalidOffset {
                    context: SourceFailureContext::Projection,
                    ..
                }
        )
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::{BytePos, FileId, Span};

    use super::*;

    #[test]
    fn projection_detection_preserves_source_context_for_every_source_failure() {
        let failures = [
            SourceError::InternalSourceMap,
            SourceError::TooLarge(usize::MAX),
            SourceError::InvalidRange { start: 9, end: 2 },
            SourceError::WrongFile {
                expected: FileId(1),
                actual: FileId(2),
            },
            SourceError::InvalidSpan(Span::new(FileId(1), 0, 1)),
            SourceError::InvalidOffset(BytePos(1)),
        ];
        for failure in &failures {
            let projection = CompileDiagnostic::from(ProjectionError::Source(failure.clone()));
            assert!(projection.is_projection_failure());
            for context in [
                SourceFailureContext::Utility,
                SourceFailureContext::Stylesheet,
                SourceFailureContext::Emission,
                SourceFailureContext::Frontend,
                SourceFailureContext::Transform,
                SourceFailureContext::MacroImport,
            ] {
                let other = CompileDiagnostic::source_failure(failure, context);
                assert!(!other.is_projection_failure());
                assert_eq!(other.code(), projection.code());
            }
        }
        assert!(
            CompileDiagnostic::from(ProjectionError::Map("invalid map".into()))
                .is_projection_failure()
        );
        assert!(!CompileDiagnostic::JavaScriptParse.is_projection_failure());
    }
}
