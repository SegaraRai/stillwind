//! Resolves known adapter failures against the attempted original source.

use std::sync::Arc;

use lean_string::ToLeanStr;
use napi::Error;
use napi_derive::napi;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::{CoordinateIndex, ModuleId, SourceFile, SourceRegistry};

use super::{HostSourceDocument, NativeConverterIdentity, NativeSourceDocument, projection_span};

/// Known source failures reported by JavaScript projection and checker adapters.
#[napi(
    discriminant = "kind",
    discriminant_case = "lowercase",
    object_to_js = false
)]
pub enum NativeSourceIssue {
    /// Original component syntax was rejected by its parser.
    Syntax { detail: String },
    /// A projection or checker buffer could not be prepared.
    Projection { detail: String },
    /// An original expression has no required checker target.
    Checker { detail: String },
    /// An assignment lost compatible declaration identities.
    Assignment { detail: String },
    /// A value does not carry a finite set of declaration identities.
    Identity { detail: String },
}

impl From<NativeSourceIssue> for CompileDiagnostic {
    fn from(value: NativeSourceIssue) -> Self {
        match value {
            NativeSourceIssue::Syntax { detail } => Self::ComponentSyntax {
                detail: (detail).into(),
            },
            NativeSourceIssue::Projection { detail } => Self::ProjectionPreparation {
                detail: (detail).into(),
            },
            NativeSourceIssue::Checker { detail } => Self::CheckerTargetUnavailable {
                detail: (detail).into(),
            },
            NativeSourceIssue::Assignment { detail } => Self::IdentityAssignment {
                detail: (detail).into(),
            },
            NativeSourceIssue::Identity { detail } => Self::IdentityUnavailable {
                detail: (detail).into(),
            },
        }
    }
}

/// Half-open UTF-16 offsets in the original document; surrogate interiors are invalid.
#[napi(object, object_to_js = false)]
pub struct NativeOriginalRange {
    pub start: u32,
    pub end: u32,
}

/// A known failure before native compilation, tied to the attempted original buffer.
#[napi(object, object_to_js = false)]
pub struct NativeSourceFailure {
    pub module_id: String,
    pub document: NativeSourceDocument,
    pub issue: NativeSourceIssue,
    /// Absent when the adapter cannot prove a precise original location.
    pub range: Option<NativeOriginalRange>,
    /// Absent for direct scripts or failures before a converter could be loaded.
    pub converter: Option<NativeConverterIdentity>,
}

impl NativeSourceFailure {
    /// Registers the original revision and validates any supplied UTF-16 range.
    /// Returns a typed failure with converter context, or rejects malformed host input.
    pub(crate) fn resolve(
        self,
        sources: &mut SourceRegistry,
    ) -> napi::Result<(ModuleId, Arc<SourceFile>, CompileError)> {
        let document = HostSourceDocument::try_from(self.document)?;
        let source = sources
            .register(
                Some(document.uri),
                document.source_map_source.as_deref().map(Into::into),
                document.code,
            )
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let mut error = if let Some(range) = self.range {
            let span = projection_span(
                &CoordinateIndex::new(&source),
                source.id(),
                range.start,
                range.end,
            )?;
            source
                .slice(span)
                .map_err(|error| Error::from_reason(error.to_string()))?;
            CompileError::new(self.issue.into(), span)
        } else {
            CompileError::for_document(self.issue.into(), &source)
        };
        error
            .notes
            .push(format_args!("Source format: {:?}", document.format).to_lean_str());
        if let Some(converter) = self.converter {
            error.notes.push(
                format_args!("Converter: {}@{}", converter.name, converter.version).to_lean_str(),
            );
        }
        Ok((ModuleId::from(self.module_id), source, error))
    }
}
