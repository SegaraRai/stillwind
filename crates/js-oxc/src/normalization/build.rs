//! Builds an analysis buffer after validating projection syntax and normalization plans.

use lean_string::{LeanString, ToLeanStr};
use oxc::allocator::Allocator;
use oxc::parser::Parser;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{AdapterKind, ScriptLanguage, ScriptModuleKind};
use stillwind_source::{FileId, ProjectionError, ProjectionIndex, ProjectionMap, SourceFile, Span};

use super::{NormalizedProjection, collect_plan, merge_omissions};
use crate::entry::source_type;

/// Creates a validated analysis view and generated checker correspondences.
///
/// `analysis_id` must differ from both input file identities. The language describes
/// the generated buffer, independently of its file extension.
///
/// # Errors
///
/// Rejects malformed maps or generated syntax and overlapping normalization plans.
pub fn normalize_projection<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
) -> Result<NormalizedProjection<'a>, CompileError> {
    if generated.id() == original.id()
        || analysis_id == generated.id()
        || analysis_id == original.id()
    {
        return Err(CompileError::for_document(
            CompileDiagnostic::ProjectionMap {
                detail: "projection sources require distinct file identities".into(),
            },
            original,
        ));
    }
    let index = ProjectionIndex::new(generated, original, map)
        .map_err(|error| CompileError::for_document(error.into(), original))?;
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        generated.text(),
        source_type(language, module_kind),
    )
    .parse();
    if let Some(diagnostic) = parsed.diagnostics.first() {
        let cause = CompileDiagnostic::ProjectionSyntax {
            detail: diagnostic.to_lean_str(),
        };
        let mapped = diagnostic.labels.first().and_then(|label| {
            let start = label.offset();
            let end = label.offset().checked_add(label.len())?;
            index
                .verify_exact(Span::new(generated.id(), start, end))
                .ok()
        });
        return Err(if let Some(mapped) = mapped {
            CompileError::new(cause, mapped.original)
        } else {
            CompileError::for_document(cause, original)
        });
    }
    let mut plan = collect_plan(generated, &index, framework.spec(), &parsed.program);
    let omitted = merge_omissions(std::mem::take(&mut plan.omitted));
    let build = || -> Result<NormalizedProjection<'a>, ProjectionError> {
        let mut output = LeanString::with_capacity(generated.text().len());
        let mut cursor = 0;
        let mut segments = Vec::new();
        for omission in omitted.into_iter().chain(std::iter::once(Span::new(
            generated.id(),
            generated.full_span().end.0,
            generated.full_span().end.0,
        ))) {
            if omission.start.0 < cursor {
                return Err(ProjectionError::Map(
                    "overlapping normalization candidates".into(),
                ));
            }
            let start = u32::try_from(output.len())
                .map_err(|_| ProjectionError::Map("normalized source too large".into()))?;
            output.push_str(generated.slice(Span::new(
                generated.id(),
                cursor,
                omission.start.0,
            ))?);
            let end = u32::try_from(output.len())
                .map_err(|_| ProjectionError::Map("normalized source too large".into()))?;
            if start != end {
                segments.push((
                    Span::new(analysis_id, start, end),
                    Span::new(generated.id(), cursor, omission.start.0),
                ));
            }
            cursor = omission.end.0;
        }
        Ok(NormalizedProjection::new(
            SourceFile::new(analysis_id, None, None, output.as_str())?,
            index,
            segments,
            plan,
        ))
    };
    build().map_err(|error| CompileError::for_document(error.into(), original))
}
