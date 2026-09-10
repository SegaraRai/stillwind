//! A validated analysis view shared by macro detection and expression comparison.

use oxc::allocator::Allocator;
use oxc::ast::ast::{Expression, ObjectProperty, PropertyKey, StaticMemberExpression};
use oxc::ast_visit::{Visit, walk};
use oxc::parser::Parser;

use stillwind_ir::ModuleIr;
use stillwind_ir::framework::{
    AdapterKind, FrameworkSpec, ProjectionDecision, ProjectionExpression,
};
use stillwind_source::{
    FileId, ProjectionError, ProjectionIndex, ProjectionMap, SourceFile, SourceProvenance, Span,
};

struct Segment {
    normalized: Span,
    projected: Span,
}

/// Normalization is only an analysis view. Output expressions come from validated
/// original spans, never from reprinting this intermediate source.
pub struct NormalizedProjection<'a> {
    pub source: SourceFile,
    index: ProjectionIndex<'a>,
    segments: Vec<Segment>,
}

impl NormalizedProjection<'_> {
    /// # Errors
    /// Rejects spans outside the analysis view or without exact original correspondence.
    pub fn original_span(&self, span: Span) -> Result<SourceProvenance, ProjectionError> {
        self.source.slice(span)?;
        let mut fragments = Vec::new();
        for segment in &self.segments {
            let start = span.start.0.max(segment.normalized.start.0);
            let end = span.end.0.min(segment.normalized.end.0);
            if start < end {
                let shift = segment.projected.start.0 - segment.normalized.start.0;
                fragments.push(Span::new(
                    segment.projected.file,
                    start + shift,
                    end + shift,
                ));
            }
        }
        self.index.verify_fragments(&fragments)
    }

    /// # Errors
    /// Fails atomically if any expression or declaration cannot be mapped.
    pub fn original_ir(
        &self,
        ir: &ModuleIr,
    ) -> Result<(ModuleIr, Vec<SourceProvenance>), ProjectionError> {
        let mut provenance = Vec::new();
        let original = ir.try_map_spans(|span| {
            let mapped = self.original_span(span)?;
            provenance.push(mapped);
            Ok::<_, ProjectionError>(mapped.original)
        })?;
        Ok((original, provenance))
    }
}

/// Creates a parser-neutral normalization plan, validates each omitted prefix
/// against converter mappings, then builds an Oxc analysis source from that plan.
///
/// # Errors
/// Rejects malformed maps, invalid syntax, overlapping plans or source identity collisions.
pub fn normalize_projection<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
) -> Result<NormalizedProjection<'a>, ProjectionError> {
    normalize_with_language(
        generated,
        original,
        map,
        framework,
        analysis_id,
        stillwind_ir::ScriptLanguage::from_path(generated.path()),
    )
}

pub(crate) fn normalize_with_language<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
    language: stillwind_ir::ScriptLanguage,
) -> Result<NormalizedProjection<'a>, ProjectionError> {
    if generated.id() == original.id()
        || analysis_id == generated.id()
        || analysis_id == original.id()
    {
        return Err(ProjectionError::Map(
            "projection sources require distinct file identities".into(),
        ));
    }
    let index = ProjectionIndex::new(generated, original, map)?;
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        generated.text(),
        super::entry::source_type(generated.path(), language),
    )
    .parse();
    if !parsed.diagnostics.is_empty() {
        return Err(ProjectionError::Map(format!(
            "invalid generated JavaScript: {}",
            parsed.diagnostics[0]
        )));
    }
    let mut visitor = NormalizationVisitor {
        source: generated,
        index: &index,
        framework: framework.spec(),
        omitted: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    let mut omitted = visitor.omitted;
    omitted.sort_by_key(|span| (span.start.0, std::cmp::Reverse(span.end.0)));
    omitted.dedup();
    let mut non_nested: Vec<Span> = Vec::new();
    for omission in omitted {
        if non_nested.last().is_some_and(|previous| {
            previous.start.0 <= omission.start.0 && previous.end.0 >= omission.end.0
        }) {
            continue;
        }
        non_nested.push(omission);
    }
    let mut output = String::with_capacity(generated.text().len());
    let mut cursor = 0;
    let mut segments = Vec::new();
    for omission in non_nested.into_iter().chain(std::iter::once(Span::new(
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
        output.push_str(generated.slice(Span::new(generated.id(), cursor, omission.start.0))?);
        let end = u32::try_from(output.len())
            .map_err(|_| ProjectionError::Map("normalized source too large".into()))?;
        if start != end {
            segments.push(Segment {
                normalized: Span::new(analysis_id, start, end),
                projected: Span::new(generated.id(), cursor, omission.start.0),
            });
        }
        cursor = omission.end.0;
    }
    Ok(NormalizedProjection {
        source: SourceFile::new(analysis_id, generated.path(), output)?,
        index,
        segments,
    })
}

struct NormalizationVisitor<'a, 'b> {
    source: &'a SourceFile,
    index: &'b ProjectionIndex<'a>,
    framework: &'static FrameworkSpec,
    omitted: Vec<Span>,
}

impl<'a> Visit<'a> for NormalizationVisitor<'_, '_> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if let Expression::Identifier(receiver) = &member.object {
            let whole = Span::new(self.source.id(), member.span.start, member.span.end);
            let property = Span::new(
                self.source.id(),
                member.property.span.start,
                member.property.span.end,
            );
            let decision = (self.framework.normalize)(ProjectionExpression::Member {
                receiver: receiver.name.as_str(),
                whole,
                property,
                optional: member.optional,
            });
            if let ProjectionDecision::Retain(retained) = decision
                && self.index.is_collapsed_prefix(whole, retained)
            {
                self.omitted
                    .push(Span::new(whole.file, whole.start.0, retained.start.0));
            }
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if !property.computed
            && !property.method
            && let PropertyKey::StaticIdentifier(key) = &property.key
            && let Expression::StaticMemberExpression(value) = &property.value
            && !value.optional
            && let Expression::Identifier(receiver) = &value.object
        {
            let whole = Span::new(self.source.id(), property.span.start, property.span.end);
            let key_span = Span::new(self.source.id(), key.span.start, key.span.end);
            let decision = (self.framework.normalize)(ProjectionExpression::ExpandedShorthand {
                receiver: receiver.name.as_str(),
                whole,
                key: key_span,
                key_name: key.name.as_str(),
                value_name: value.property.name.as_str(),
            });
            if let ProjectionDecision::Retain(retained) = decision
                && self.index.is_collapsed_wrapper(whole, retained)
            {
                if whole.start.0 < retained.start.0 {
                    self.omitted
                        .push(Span::new(whole.file, whole.start.0, retained.start.0));
                }
                if retained.end.0 < whole.end.0 {
                    self.omitted
                        .push(Span::new(whole.file, retained.end.0, whole.end.0));
                }
            }
        }
        walk::walk_object_property(self, property);
    }
}
