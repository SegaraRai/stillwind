//! Checked queries from retained analysis text to authored and generated coordinates.

use std::collections::BTreeMap;

use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{CheckerBinding, CheckerRole, CheckerSpan, ModuleIr};
use stillwind_source::{ProjectionError, ProjectionIndex, SourceFile, SourceProvenance, Span};

/// A retained run with equal byte lengths in both buffers.
struct Segment {
    normalized: Span,
    projected: Span,
}

/// An analysis view and the validated generated expressions that it represents.
///
/// Output edits use original spans. This view borrows the projection sources and
/// owns the analysis source tagged with the caller's distinct file identity.
pub struct NormalizedProjection<'a> {
    /// Source for macro extraction and expression comparison.
    pub source: SourceFile,
    index: ProjectionIndex<'a>,
    segments: Vec<Segment>,
    expressions: BTreeMap<(u32, u32), Span>,
    producers: BTreeMap<(u32, u32), Span>,
    ignored: Vec<Span>,
    bindings: Vec<CheckerBinding>,
}

impl<'a> NormalizedProjection<'a> {
    pub(super) fn new(
        source: SourceFile,
        index: ProjectionIndex<'a>,
        segments: Vec<(Span, Span)>,
        plan: super::NormalizationPlan,
    ) -> Self {
        let mut normalized = Self {
            source,
            index,
            segments: segments
                .into_iter()
                .map(|(normalized, projected)| Segment {
                    normalized,
                    projected,
                })
                .collect(),
            expressions: BTreeMap::new(),
            producers: BTreeMap::new(),
            ignored: plan.ignored,
            bindings: plan.bindings,
        };
        normalized.record_expressions(plan.expressions);
        normalized
    }

    /// Converts a mapping failure into a diagnostic in the original document.
    pub(crate) fn failure(&self, error: ProjectionError, original: &SourceFile) -> CompileError {
        let mapped = match &error {
            ProjectionError::Unmapped(span) => self.original_span(*span).ok(),
            _ => None,
        };
        if let Some(mapped) = mapped {
            CompileError::new(
                CompileDiagnostic::CheckerTargetUnavailable {
                    detail: "the generated AST has no target for the required checker operation"
                        .into(),
                },
                mapped.original,
            )
        } else {
            CompileError::for_document(error.into(), original)
        }
    }

    fn fragments(&self, span: Span) -> Vec<Span> {
        self.segments
            .iter()
            .filter_map(|segment| {
                let start = span.start.0.max(segment.normalized.start.0);
                let end = span.end.0.min(segment.normalized.end.0);
                if start < end
                    || (span.start == span.end
                        && segment.normalized.start <= span.start
                        && span.end <= segment.normalized.end)
                {
                    let shift = segment.projected.start.0 - segment.normalized.start.0;
                    Some(Span::new(
                        segment.projected.file,
                        start + shift,
                        end + shift,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Maps retained analysis text to the exact authored text.
    ///
    /// # Errors
    ///
    /// Rejects invalid analysis spans and ambiguous or nonmatching correspondence.
    pub fn original_span(&self, span: Span) -> Result<SourceProvenance, ProjectionError> {
        self.source.slice(span)?;
        self.index.verify_fragments(&self.fragments(span))
    }

    /// Returns the generated AST node for a checker operation on an analysis span.
    ///
    /// # Errors
    ///
    /// Rejects spans without exact original correspondence or the required AST role.
    pub fn checker_span(
        &self,
        span: Span,
        role: CheckerRole,
    ) -> Result<CheckerSpan, ProjectionError> {
        let mut provenance = self.original_span(span)?;
        provenance.projected = match role {
            CheckerRole::Expression => self.expressions.get(&(span.start.0, span.end.0)).copied(),
            CheckerRole::Producer => self.producers.get(&(span.start.0, span.end.0)).copied(),
            CheckerRole::ModuleSpecifier => self
                .index
                .verify_exact(provenance.projected)
                .ok()
                .map(|mapped| mapped.projected),
        }
        .ok_or(ProjectionError::Unmapped(span))?;
        Ok(CheckerSpan { provenance, role })
    }

    /// Generated references whose bindings the checker must protect from author scopes.
    pub fn checker_bindings(&self) -> &[CheckerBinding] {
        &self.bindings
    }

    fn record_expressions(&mut self, expressions: Vec<(Span, bool)>) {
        for (generated_span, call) in expressions {
            let retained: Vec<_> = self
                .segments
                .iter()
                .filter_map(|segment| {
                    let start = generated_span.start.0.max(segment.projected.start.0);
                    let end = generated_span.end.0.min(segment.projected.end.0);
                    (start < end).then(|| {
                        (
                            start - segment.projected.start.0 + segment.normalized.start.0,
                            end - segment.projected.start.0 + segment.normalized.start.0,
                        )
                    })
                })
                .collect();
            if let Some((first, last)) = retained.first().zip(retained.last()) {
                let key = (first.0, last.1);
                let span = Span::new(self.source.id(), key.0, key.1);
                if self.original_span(span).is_ok() {
                    // Parents are visited after children, so a validated wrapper wins
                    // over its retained argument when both represent the same text.
                    self.expressions.insert(key, generated_span);
                    if call {
                        self.producers.insert(key, generated_span);
                    }
                }
            }
        }
    }

    fn ignored_reference(&self, span: Span) -> bool {
        let fragments = self.fragments(span);
        fragments
            .first()
            .zip(fragments.last())
            .is_some_and(|(first, last)| {
                self.ignored
                    .iter()
                    .any(|ignored| ignored.start <= first.start && last.end <= ignored.end)
            })
    }

    /// Maps IR atomically and excludes validated synthetic import reads.
    ///
    /// Unmapped reads outside synthetic regions remain import-deletion guards.
    ///
    /// # Errors
    ///
    /// Rejects any expression or declaration without exact original correspondence.
    pub fn original_ir(
        &self,
        ir: &ModuleIr,
    ) -> Result<(ModuleIr, Vec<SourceProvenance>), ProjectionError> {
        let mut provenance = Vec::new();
        let mut editable = ir.clone();
        for import in &mut editable.imports.macros {
            for specifier in &mut import.specifiers {
                specifier.references.clear();
            }
        }
        let mut original = editable.try_map_spans(|span| {
            if span.file != self.source.id() {
                return Ok(span);
            }
            let mapped = self.original_span(span)?;
            provenance.push(mapped);
            Ok::<_, ProjectionError>(mapped.original)
        })?;
        for (target, source) in original.imports.macros.iter_mut().zip(&ir.imports.macros) {
            for (target, source) in target.specifiers.iter_mut().zip(&source.specifiers) {
                for reference in &source.references {
                    if self.ignored_reference(*reference) {
                        continue;
                    }
                    if let Ok(mapped) = self.original_span(*reference) {
                        target.references.push(mapped.original);
                        provenance.push(mapped);
                    } else {
                        target.has_unmapped_references = true;
                    }
                }
                target.references.sort_by_key(|span| (span.start, span.end));
                target.references.dedup();
            }
        }
        Ok((original, provenance))
    }
}
